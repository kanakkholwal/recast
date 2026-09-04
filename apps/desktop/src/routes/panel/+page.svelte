<script lang="ts">
import { page } from "$app/state";
import { checkCapability, loadCapabilities } from "$lib/capabilities";
import {
	type AudioDeviceInfo,
	type CameraValidationResult,
	CAPTURE_INTENT_CHANGED_EVENT,
	type CaptureIntentState,
	excludeWindowFromCapture,
	getAudioDevices,
	getCameraDevices,
	getCaptureIntent,
	getDisplays,
	getLastSource,
	pauseRecording,
	type RecordingOptions,
	refreshTray,
	resumeRecording,
	setCaptureIntent,
	setLastSource,
	startRecording,
	stopRecording,
	validateCameraSource,
} from "$lib/ipc";
import { isBrowserDeviceId } from "$lib/runtime/device-id";
import { profilesStore } from "$lib/stores/profiles.svelte";
import { recordingCountdown } from "$lib/stores/recording-countdown.svelte";
import { spawnOverlayWindow } from "$lib/windows/spawn-overlay";
import {
	type BrowserCamera,
	isVirtualCameraLabel,
} from "@recast/editor/lib/camera/browser-devices";
import {
	loadRecordingFps,
	loadRecordingQuality,
	type RecordingProfile,
	resolveCamera,
	resolveMic,
} from "@recast/editor/lib/profiles";
import {
	AlertTriangle,
	AppWindow,
	Camera,
	CameraOff,
	Check,
	ChevronDown,
	Crop,
	GripVertical,
	LoaderCircle,
	Mic,
	MicOff,
	Monitor,
	PauseFilled,
	PlayFilled,
	RecordFilled,
	SlidersHorizontal as SlidersIcon,
	SquareFilled,
	Volume,
	VolumeOff,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ButtonGroup } from "@recast/ui/button-group";
import { emit, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "@tauri-apps/plugin-dialog";
import { platform } from "@tauri-apps/plugin-os";
import { onMount } from "svelte";
import { cubicOut } from "svelte/easing";
import { Tween } from "svelte/motion";
import { fade, scale } from "svelte/transition";
import LogoWave from "./LogoWave.svelte";
import {
	buildCaptureIntent,
	canonicalIntent,
	clampFpsToDisplay,
	deviceOutcome,
	formatRecordingTimer,
	intentToTargetType,
	lastSourceToTarget,
	smoothMicLevel,
	sourceFromIntent,
	type TargetSource,
	targetToLastSource,
} from "./panel.logic";

// Too small for its own Toaster: emit `ui:toast` for the main window; alert() if emit throws.
type ToastLevel = "error" | "warning" | "info" | "success";
function notify(level: ToastLevel, message: string, duration: number | undefined = undefined) {
	emit("ui:toast", { level, message, duration }).catch((err) => {
		console.error("ui:toast emit failed, falling back to alert", err);
		window.alert(message);
	});
}

let selectedSource: TargetSource | null = $state(null);
let isRecording = $state(false);
// True between the countdown ending and startRecording resolving, so `phase` never dips to idle.
let isStarting = $state(false);
// Guards a second stop click while stop_recording is in flight (slow on macOS) from erroring spuriously.
let isStopping = $state(false);
// Post-stop feedback ON the panel: without it the bar snaps to idle with no "did it save?" confirmation, and failures went to a main-window toast hidden behind the capture.
let saveState = $state<"idle" | "saving" | "saved" | "failed">("idle");
let saveError = $state<string | null>(null);
let saveResetTimer: ReturnType<typeof setTimeout> | undefined;
let recordingStartTime: number | null = $state(null);
let now = $state(Date.now());

// `countdownValue` is the live integer tick; `countdownProgress` (1 to 0) drives the depleting ring.
let countdownValue = $state<number | null>(null);
let countdownProgress = $state(1);
let countdownRaf: number | null = null;
// Ring circumference (r=16 in the 36×36 viewBox); dash offset = C × (1 − progress).
const RING_C = 2 * Math.PI * 16;

// The window keeps its launch size: a centered always-on-top window can't resize and reposition atomically.
const BAR_W_IDLE = 488;

let barContentEl = $state<HTMLElement | null>(null);
let measuredBarW = $state(BAR_W_IDLE);
const barWidth = new Tween(BAR_W_IDLE, { duration: 260, easing: cubicOut });
// Snap the very first measurement instead of animating from the seed value.
let barFirstMeasure = true;
const prefersReducedMotion =
	typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

// Linux (tao) doesn't honor always-on-top for these overlay windows; matches the guards in $lib/ipc.
const IS_LINUX = platform() === "linux";

// Border box so the bar wraps exactly, rounded to whole px so sub-pixel jitter can't retrigger.
$effect(() => {
	if (!barContentEl) return;
	const ro = new ResizeObserver((entries) => {
		const entry = entries[0];
		if (!entry) return;
		const w = Math.round(entry.borderBoxSize?.[0]?.inlineSize ?? entry.contentRect.width);
		if (w > 0) measuredBarW = w;
	});
	ro.observe(barContentEl);
	return () => ro.disconnect();
});

$effect(() => {
	if (measuredBarW <= 0) return;

	if (barFirstMeasure) {
		// First paint: size the bar to the content with no animation.
		barWidth.set(measuredBarW, { duration: 0 });
		barFirstMeasure = false;
		return;
	}

	// Instant under reduced motion. The window never moves; the bar morphs centered within it.
	if (prefersReducedMotion) barWidth.set(measuredBarW, { duration: 0 });
	else barWidth.target = measuredBarW;
});

// `idle` = full controls; `countdown` = number + cancel; `recording` = transport.
const phase = $derived.by<"idle" | "countdown" | "recording" | "finalizing">(() => {
	if (saveState !== "idle") return "finalizing";
	if (isRecording || isStarting) return "recording";
	if (countdownValue !== null) return "countdown";
	return "idle";
});

// Mirrors the recording flag to the tray label; a no-op if tray init failed.
$effect(() => {
	void refreshTray(isRecording);
});

// `pausedAccumMs` banks completed pauses, `pausedSince` marks one in progress; the timer subtracts both.
let isPaused = $state(false);
let pausedAccumMs = $state(0);
let pausedSince: number | null = $state(null);

// The camera keeps recording through a pause, so a forgotten pause quietly wastes disk.
const PAUSE_PROMPT_INTERVAL_MS = 5 * 60 * 1000;
let pausePromptOpen = $state(false);
let lastPausePromptAt: number | null = $state(null);

// Device toggles
let systemAudioOn = $state(true);
let micOn = $state(false);
// Real 0..1 input level fed by Rust's `mic-level` event during recording (perceptual-curved + smoothed), so the meter reflects captured audio rather than a decorative loop.
let micLevel = $state(0);
// Per-bar scale so the meter reads like an equalizer, not a flat block.
const METER_BARS = [0.62, 1, 0.82, 0.9];
let cameraOn = $state(false);

// Selected devices
let selectedMicId = $state<string | null>(null);
let selectedMicName = $state("Default");
let selectedCameraId = $state<string | null>(null);
let selectedCameraName = $state("Default");
let cameraValidation = $state<CameraValidationResult | null>(null);

// The panel can't host a toast, so resolution outcomes live in tooltips until the next apply or toggle.
let micWarning = $state<string | null>(null);
let cameraWarning = $state<string | null>(null);

// The most severe active device problem, surfaced as an inline chip in the idle bar rather than a hover-only title.
const deviceIssue = $derived.by<{ level: "error" | "warning"; text: string } | null>(() => {
	if (cameraValidation?.status === "error") {
		return { level: "error", text: cameraValidation.statusMessage ?? "Camera unavailable" };
	}
	if (cameraWarning) return { level: "warning", text: cameraWarning };
	if (micWarning) return { level: "warning", text: micWarning };
	return null;
});

// Refreshed on every profile resolve, so the resolver sees current hardware (USB devices come and go).
let mics = $state<AudioDeviceInfo[]>([]);
let cameras = $state<BrowserCamera[]>([]);

// Manual toggle overrides don't clear this; the chip is only a 'last applied' marker.
let activeProfileId = $state<string | null>(null);
// Brief highlight after a successful apply, so there is a confirmation cue without a toast.
let profileFlash = $state(false);
let profileFlashTimer: ReturnType<typeof setTimeout> | null = null;

const activeProfile = $derived(activeProfileId ? profilesStore.findById(activeProfileId) : null);

// Derived off `activeProfile`, not snapshotted, so a live cross-window edit updates the countdown at once.
const countdownSeconds = $derived(activeProfile?.countdown ?? recordingCountdown.value);

// --- Capture-intent sync: `lastIntent` is the last value sent or received, and comparing before writing breaks the echo loop.
let lastIntent = $state<CaptureIntentState | null>(null);
let intentSyncReady = $state(false);

function buildIntentFromPanel(): CaptureIntentState {
	return buildCaptureIntent(lastIntent, {
		source: selectedSource,
		systemAudio: systemAudioOn,
		micOn,
		micDeviceId: selectedMicId,
		cameraOn,
		cameraName: selectedCameraName,
	});
}

// Apply an externally-set intent (a CLI `select`/`set`) to the panel state.
function applyIntentToPanel(intent: CaptureIntentState) {
	const source = sourceFromIntent(intent);
	if (source) {
		selectedSource = source;
		// Enrich a monitor with its real name + refresh for the label and fps cap.
		if (source.type === "monitor") {
			const wantId = intent.targetId;
			getDisplays()
				.then((displays) => {
					const d = displays.find((x) => x.id === wantId);
					if (d && selectedSource?.id === wantId) {
						selectedSource = {
							...selectedSource,
							label: d.isPrimary ? "Primary Display" : `Display ${d.id}`,
							refreshHz: d.refreshHz || undefined,
						};
					}
				})
				.catch(() => undefined);
		}
	}

	systemAudioOn = intent.options.systemAudio ?? true;

	// Mic ids are the same Rust audio ids the panel already uses.
	if (intent.options.microphone) {
		micOn = true;
		selectedMicId = intent.options.microphoneDeviceId ?? selectedMicId;
		selectedMicName = mics.find((m) => m.id === selectedMicId)?.name ?? selectedMicName;
		micWarning = null;
	} else {
		micOn = false;
	}

	// The intent carries the DirectShow name; match a browser device by label to drive the preview.
	if (intent.options.camera) {
		const name = intent.options.cameraDeviceId ?? selectedCameraName;
		cameraOn = true;
		selectedCameraName = name;
		const match = cameras.find((c) => c.label === name);
		if (match) {
			selectedCameraId = match.deviceId;
			void refreshCameraValidation(match.deviceId);
			openCameraPreview(match.label);
		}
		cameraWarning = null;
	} else if (cameraOn) {
		cameraOn = false;
		cameraValidation = null;
		closeCameraPreview();
	}
}

// Adopt a source the CLI staged before the panel opened, then seed the guard so the first push keeps it.
async function initIntentSync() {
	try {
		const intent = await getCaptureIntent();
		lastIntent = intent;
		if (intentToTargetType(intent.targetType)) applyIntentToPanel(intent);
		intentSyncReady = true;
	} catch {
		// Non-Tauri preview or older build: leave sync off.
	}
}

// Push on change, unless it already matches (our own echo, or a value we just applied).
$effect(() => {
	const next = buildIntentFromPanel();
	if (!intentSyncReady) return;
	// Canonical compare: the backend echo omits nulls we send, so a raw JSON compare loops forever.
	if (canonicalIntent(next) === canonicalIntent(lastIntent)) return;
	lastIntent = next;
	setCaptureIntent(next).catch(() => undefined);
});

async function refreshCameraValidation(deviceId: string | null) {
	if (!deviceId) {
		cameraValidation = null;
		return;
	}

	// The Rust validator only knows DirectShow names, so skip browser MediaDevices hex ids.
	if (isBrowserDeviceId(deviceId)) {
		cameraValidation = {
			id: deviceId,
			name: selectedCameraName,
			status: "ready",
			statusMessage: null,
			probedAtUnixMs: Date.now(),
		};
		return;
	}

	try {
		cameraValidation = await validateCameraSource(deviceId);
	} catch {
		cameraValidation = {
			id: deviceId,
			name: selectedCameraName,
			status: "unknown",
			statusMessage: "Camera validation could not complete.",
			probedAtUnixMs: Date.now(),
		};
	}
}

onMount(() => {
	const html = document.documentElement;
	const body = document.body;
	html.style.background = "transparent";
	html.style.overflow = "hidden";
	html.style.scrollbarGutter = "auto";
	(html.style as CSSStyleDeclaration & { scrollbarWidth?: string }).scrollbarWidth = "none";
	body.style.background = "transparent";
	body.style.overflow = "hidden";
	body.style.margin = "0";

	const timer = window.setInterval(() => {
		if (isRecording) now = Date.now();
	}, 1000);

	const unlistenSource = listen<TargetSource>("source-selected", (event) => {
		selectedSource = event.payload;
		// Persist for next launch.
		setLastSource(targetToLastSource(event.payload)).catch(() => undefined);
	});

	// Listen for device selection from picker windows
	const unlistenDevice = listen<{
		type: string;
		id: string | null;
		name: string;
	}>("device-selected", (event) => {
		const { type, id, name } = event.payload;
		if (type === "mic") {
			if (id) {
				micOn = true;
				selectedMicId = id;
				selectedMicName = name;
			} else {
				micOn = false;
			}
		} else if (type === "camera") {
			if (id) {
				cameraOn = true;
				selectedCameraId = id;
				selectedCameraName = name;
				void refreshCameraValidation(id);
				openCameraPreview(name);
			} else {
				cameraOn = false;
				cameraValidation = null;
				closeCameraPreview();
			}
		}
	});

	// Profile-picker window applies through the same path as ⌘1-⌘8 shortcuts.
	const unlistenProfile = listen<{ id: string }>("profile-selected", (event) => {
		const target = profilesStore.findById(event.payload.id);
		if (target) handleProfileSwitch(target);
	});

	// Prefer the last-used source from persisted config; fall back to the primary display.
	getLastSource()
		.then((last) => {
			if (last) {
				selectedSource = lastSourceToTarget(last);
				// Look up the restored monitor's refresh rate so fps clamping knows the ceiling without a probe.
				if (selectedSource?.type === "monitor") {
					const restoredId = selectedSource.id;
					getDisplays()
						.then((displays) => {
							const hz = displays.find((d) => d.id === restoredId)?.refreshHz;
							if (hz && selectedSource && selectedSource.id === restoredId) {
								selectedSource = {
									...selectedSource,
									refreshHz: hz,
								};
							}
						})
						.catch(() => undefined);
				}
				return;
			}
			return getDisplays().then((displays) => {
				if (displays.length > 0 && !selectedSource) {
					const d = displays[0];
					selectedSource = {
						type: "monitor",
						id: d.id,
						label: d.isPrimary ? "Primary Display" : `Display ${d.id}`,
						refreshHz: d.refreshHz || undefined,
					};
				}
			});
		})
		.catch(() => undefined)
		// After the last source is restored, so 'screen' wins and the picker modes open on a sensible base.
		.finally(() => void applyCaptureIntent(page.url.searchParams.get("intent")));

	// The panel may already be open when a mode tile is clicked, so the intent arrives as an event.
	const unlistenIntent = listen<{ intent: string }>(
		"panel-capture-intent",
		(event) => void applyCaptureIntent(event.payload.intent),
	);

	// Reflect the preview window closing by any means, so the camera toggle can't stay lit.
	const unlistenCameraClosed = listen("camera-preview-closed", () => {
		if (isRecording || !cameraOn) return;
		cameraOn = false;
		cameraValidation = null;
		cameraWarning = null;
	});

	profilesStore.hydrate();

	// Load devices/profile, then adopt any CLI-staged intent and enable sync.
	void initDevicesAndProfile().then(() => initIntentSync());
	// Warm the capability probe so the first device toggle resolves instantly.
	void loadCapabilities();

	// Apply external intent edits (from `recast select`/`set`) to the panel.
	const unlistenIntentChanged = listen<CaptureIntentState>(
		CAPTURE_INTENT_CHANGED_EVENT,
		(event) => {
			if (isRecording) return; // selections are locked during a take
			const incoming = event.payload;
			// Canonical compare (see the push effect): ignore our own echo so we don't re-apply and loop.
			if (canonicalIntent(incoming) === canonicalIntent(lastIntent)) return;
			lastIntent = incoming;
			applyIntentToPanel(incoming);
		},
	);

	// Reflect a recording the panel did NOT start (CLI, or a --timeout auto-stop); panel takes set the flags first.
	const unlistenRecStarted = listen<{ startedAtUnixMs: number }>("recording:started", (event) => {
		if (isRecording || isStarting) return;
		// An external (CLI/timeout) start inside the "Saved" window must clear it, or the transport hides behind the finalizing UI.
		dismissSave();
		clearCountdown();
		now = Date.now();
		recordingStartTime = event.payload.startedAtUnixMs ?? now;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		isRecording = true;
	});
	const unlistenRecStopped = listen("recording:stopped", () => {
		if (!isRecording) return;
		recordingStartTime = null;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		micLevel = 0;
		isRecording = false;
		isStopping = false;
		closeCameraPreview();
		emit("refresh-recordings");
	});
	// The recording saved, so surface non-fatal capture issues rather than let a missing track surprise the editor.
	const unlistenRecWarnings = listen<string[]>("recording:warnings", (event) => {
		const messages = event.payload ?? [];
		if (messages.length > 0) {
			notify("warning", messages.join("\n"), 8000);
		}
	});
	// Rust emits the mic RMS ~15Hz while recording; smoothMicLevel curves + smooths it into a meter value.
	const unlistenMicLevel = listen<number>("mic-level", (event) => {
		micLevel = smoothMicLevel(micLevel, event.payload ?? 0);
	});

	window.addEventListener("keydown", handleGlobalShortcut);

	// Intercept close during a live recording so it's finalized, not lost.
	const closeReq = getCurrentWindow().onCloseRequested((event) => {
		if (isClosing || !isRecording) return;
		event.preventDefault();
		void finalizeAndClose();
	});

	// Tray "Start/Stop Recording" routes here when the panel is open.
	const unlistenTrayToggle = listen("tray:record-toggle", () => {
		void toggleRecording();
	});

	// Rust emits this only while a recording is active, so the panel is the right owner.
	const unlistenGlobalPause = listen("global-shortcut:toggle-pause", () => {
		void togglePause();
	});

	// Tray "Pause/Resume Recording" routes here (shown only while recording).
	const unlistenTrayPause = listen("tray:pause-toggle", () => {
		void togglePause();
	});

	return () => {
		window.clearInterval(timer);
		if (profileFlashTimer) clearTimeout(profileFlashTimer);
		clearCountdown();
		unlistenSource.then((fn) => fn());
		unlistenDevice.then((fn) => fn());
		unlistenProfile.then((fn) => fn());
		closeReq.then((fn) => fn());
		unlistenTrayToggle.then((fn) => fn());
		unlistenGlobalPause.then((fn) => fn());
		unlistenTrayPause.then((fn) => fn());
		unlistenIntent.then((fn) => fn());
		unlistenCameraClosed.then((fn) => fn());
		unlistenIntentChanged.then((fn) => fn());
		unlistenRecStarted.then((fn) => fn());
		unlistenRecStopped.then((fn) => fn());
		unlistenRecWarnings.then((fn) => fn());
		unlistenMicLevel.then((fn) => fn());
		window.removeEventListener("keydown", handleGlobalShortcut);
	};
});

// Loads devices, then applies the default profile, or seeds defaults (default mic, first real camera, system audio).
async function initDevicesAndProfile() {
	// Cameras come from Rust: offering one the backend cannot open is a dead end.
	const [audioDevices, videoDevices] = await Promise.all([
		getAudioDevices().catch(() => [] as AudioDeviceInfo[]),
		getCameraDevices()
			.then((found) =>
				found.map((camera) => ({
					deviceId: camera.name,
					label: camera.name,
					groupId: "",
					isVirtual: isVirtualCameraLabel(camera.name),
				})),
			)
			.catch(() => [] as BrowserCamera[]),
	]);
	mics = audioDevices;
	cameras = videoDevices;

	// Seed defaults even when applying a profile, so a later manual toggle has something to use.
	const defaultMic = audioDevices.find((d) => d.isDefault) ?? audioDevices[0];
	if (defaultMic) {
		selectedMicId = defaultMic.id;
		selectedMicName = defaultMic.name;
	}
	const defaultCam = videoDevices.find((c) => !c.isVirtual) ?? videoDevices[0];
	if (defaultCam) {
		selectedCameraId = defaultCam.deviceId;
		selectedCameraName = defaultCam.label;
		void refreshCameraValidation(defaultCam.deviceId);
	}

	// Profiles load async now, so wait before applying the default; the seeded devices stand until then.
	await profilesStore.hydrate();
	if (!profilesStore.enabled) return;
	const def = profilesStore.default();
	if (!def) return;
	applyProfile(def);
}

/**
 * Apply a profile to the panel state (toggles + device selections),
 * resolving devices against the current hardware list. Fallback / missing
 * outcomes are recorded into `micWarning` / `cameraWarning` so the device
 * button tooltips surface them on hover (Sonner toasts would overflow the
 * 44px-tall panel window).
 */
function applyProfile(profile: RecordingProfile) {
	systemAudioOn = profile.systemAudio;

	const mic = deviceOutcome(resolveMic(profile, mics), profile.name, "mic", (d) => d.name);
	micOn = mic.on;
	micWarning = mic.warning;
	if (mic.device) {
		selectedMicId = mic.device.id;
		selectedMicName = mic.device.name;
	}

	const wasCameraOn = cameraOn;
	const camera = deviceOutcome(
		resolveCamera(profile, cameras),
		profile.name,
		"camera",
		(d) => d.label,
	);
	cameraOn = camera.on;
	cameraWarning = camera.warning;
	if (camera.device) {
		selectedCameraId = camera.device.deviceId;
		selectedCameraName = camera.device.label;
		void refreshCameraValidation(camera.device.deviceId);
		openCameraPreview(camera.device.label);
	} else {
		cameraValidation = null;
		// `missing` tears down unconditionally since the profile asked for a camera; `none` only closes what was open.
		if (camera.kind === "missing" || wasCameraOn) closeCameraPreview();
	}

	// The countdown follows the profile live via `countdownSeconds`, so setting the id is all that is needed.
	activeProfileId = profile.id;
}

function handleProfileSwitch(profile: RecordingProfile) {
	if (isRecording) return;
	applyProfile(profile);
	// Brief highlight on the profile button, so confirmation needs no toast.
	if (profileFlashTimer) clearTimeout(profileFlashTimer);
	profileFlash = true;
	profileFlashTimer = setTimeout(() => {
		profileFlash = false;
		profileFlashTimer = null;
	}, 1400);
}

function handleGlobalShortcut(e: KeyboardEvent) {
	// During the pre-roll: Esc aborts, Enter/Space skips straight to capture.
	if (countdownValue !== null) {
		if (e.key === "Escape") {
			e.preventDefault();
			cancelCountdown();
		} else if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			startNow();
		}
		return;
	}
	if (isRecording) return;
	const meta = e.metaKey || e.ctrlKey;
	if (!meta || e.shiftKey || e.altKey) return;
	if (!profilesStore.enabled) return;
	const digit = parseInt(e.key, 10);
	if (Number.isFinite(digit) && digit >= 1 && digit <= 8) {
		const profile = profilesStore.profiles[digit - 1];
		if (profile) {
			e.preventDefault();
			handleProfileSwitch(profile);
		}
	}
}

function openSourceSelector(
	tab: "monitor" | "window" | "region" | undefined = undefined,
	autostart = false,
) {
	if (isRecording) return;
	const params = new URLSearchParams();
	if (tab) params.set("tab", tab);
	if (autostart) params.set("autostart", "1");
	const qs = params.toString();
	void spawnOverlayWindow("source-selector", {
		url: qs ? `/select?${qs}` : "/select",
		title: "Select Source",
		width: 560,
		height: 440,
		center: true,
		decorations: false,
		transparent: true,
		shadow: false,
		resizable: false,
	});
}

// Arrives via the panel URL on first launch or the `panel-capture-intent` event; never touches a live recording.
async function applyCaptureIntent(intent: string | null | undefined) {
	if (!intent || isRecording) return;
	switch (intent) {
		case "screen": {
			// Full screen is unambiguous, so pick the primary display instead of making the user confirm.
			try {
				const displays = await getDisplays();
				const primary = displays.find((d) => d.isPrimary) ?? displays[0];
				if (primary) {
					selectedSource = {
						type: "monitor",
						id: primary.id,
						label: primary.isPrimary ? "Primary Display" : `Display ${primary.id}`,
						refreshHz: primary.refreshHz || undefined,
					};
				}
			} catch {
				// Leave whatever the panel restored.
			}
			break;
		}
		case "window":
			openSourceSelector("window");
			break;
		case "region":
			openSourceSelector("region", true);
			break;
		case "camera":
			// No webcam-only source exists, so add the camera overlay to the current screen source.
			if (!cameraOn) void toggleCamera();
			break;
	}
}

function openProfilePicker() {
	if (isRecording) return;
	void spawnOverlayWindow("profile-picker", {
		url: `/profile-picker?selected=${activeProfileId ?? ""}`,
		title: "Switch profile",
		width: 320,
		height: 380,
		center: true,
		decorations: false,
		transparent: true,
		shadow: false,
		resizable: false,
	});
}

function openDevicePicker(type: "mic" | "camera") {
	if (isRecording) return;
	const selected = type === "mic" ? selectedMicId : selectedCameraId;
	void spawnOverlayWindow(`device-picker-${type}`, {
		url: `/device-picker?type=${type}&selected=${selected ?? ""}`,
		title: `Select ${type === "mic" ? "Microphone" : "Camera"}`,
		width: 320,
		height: 340,
		center: true,
		decorations: false,
		transparent: true,
		shadow: false,
		resizable: false,
	});
}

/** `name` is the camera's friendly label: Rust resolves the device by name, and
 *  the browser's deviceId hash means nothing to Media Foundation. */
function openCameraPreview(name: string) {
	WebviewWindow.getByLabel("camera-preview").then(async (existing) => {
		if (existing) {
			await existing.close();
		}
		const win = new WebviewWindow("camera-preview", {
			url: `/camera-preview?deviceId=${encodeURIComponent(name)}`,
			title: "Camera",
			width: 240,
			height: 240,
			decorations: false,
			transparent: true,
			shadow: false,
			alwaysOnTop: !IS_LINUX,
			resizable: true,
			skipTaskbar: true,
			x: 40,
			y: 40,
		});
		// Must be excluded or DXGI Desktop Duplication bakes the bubble into the recording; the HWND needs `tauri://created`.
		win.once("tauri://created", () => {
			excludeWindowFromCapture("camera-preview").catch((err) =>
				console.warn("camera preview exclusion failed:", err),
			);
		});
	});
}

function closeCameraPreview() {
	emit("camera-recording-stopped");
	emit("camera-stop");
	WebviewWindow.getByLabel("camera-preview").then(async (existing) => {
		if (existing) await existing.close();
	});
}

function closePanel() {
	closeCameraPreview();
	getCurrentWindow().close();
}

async function toggleMic() {
	if (isRecording) return;
	micWarning = null;
	if (micOn) {
		micOn = false;
		return;
	}
	const verdict = await checkCapability("microphone", "Microphone");
	if (!verdict.ok) {
		notify("warning", verdict.message);
		return;
	}
	openDevicePicker("mic");
}

async function toggleCamera() {
	if (isRecording) return;
	cameraWarning = null;
	if (cameraOn) {
		cameraOn = false;
		closeCameraPreview();
		return;
	}
	const verdict = await checkCapability("camera", "Webcam");
	if (!verdict.ok) {
		notify("warning", verdict.message);
		return;
	}
	openDevicePicker("camera");
}

async function toggleSystemAudio() {
	// Turning it off is always fine; only gate turning it on.
	if (systemAudioOn) {
		systemAudioOn = false;
		return;
	}
	const verdict = await checkCapability("systemAudio", "System audio");
	if (!verdict.ok) {
		notify("warning", verdict.message);
		return;
	}
	systemAudioOn = true;
}

function clearCountdown() {
	if (countdownRaf !== null) {
		cancelAnimationFrame(countdownRaf);
		countdownRaf = null;
	}
	countdownValue = null;
	countdownProgress = 1;
}

function cancelCountdown() {
	clearCountdown();
}

/** Skip the remaining pre-roll and start capturing right now. */
function startNow() {
	if (countdownValue === null) return;
	// Enter 'starting' before clearing the countdown so `phase` never dips through idle while the IPC resolves.
	isStarting = true;
	clearCountdown();
	void startActualRecording();
}

/**
 * Start path for the Record button. With a countdown configured, run a
 * deadline-based pre-roll in the panel first (cancelable via Esc / Cancel,
 * skippable via the ring / Enter) then fire the real capture. With countdown
 * off, start immediately.
 *
 * The loop is driven by `requestAnimationFrame` against a fixed end time
 * rather than a 1s `setInterval`: the integer readout stays accurate (no
 * drift from timer slop) and the ring depletes smoothly at display refresh
 * rate. rAF also auto-pauses if the panel is hidden.
 */
function beginRecording() {
	if (!selectedSource || isRecording || isStarting || countdownValue !== null) return;
	// A tray/shortcut start can land inside the post-stop "Saved" window; clear it so the finalizing UI doesn't paint over the new countdown.
	dismissSave();
	const secs = countdownSeconds;
	if (secs <= 0) {
		void startActualRecording();
		return;
	}
	const totalMs = secs * 1000;
	const endsAt = Date.now() + totalMs;
	countdownValue = secs;
	countdownProgress = 1;
	const tick = () => {
		const remaining = endsAt - Date.now();
		if (remaining <= 0) {
			// Bridge to recording via `isStarting` so the phase never falls back to idle during the start IPC.
			isStarting = true;
			clearCountdown();
			void startActualRecording();
			return;
		}
		countdownValue = Math.ceil(remaining / 1000);
		countdownProgress = remaining / totalMs;
		countdownRaf = requestAnimationFrame(tick);
	};
	countdownRaf = requestAnimationFrame(tick);
}

async function toggleRecording() {
	// A tray toggle or shortcut can still land here mid-countdown; treat it as cancel.
	if (countdownValue !== null) {
		cancelCountdown();
		return;
	}
	// Mid-handoff (countdown done, start IPC in flight): ignore the toggle so a stray click can't start a new countdown.
	if (isStarting) return;
	if (!isRecording) {
		beginRecording();
		return;
	}
	// A stop is already in flight; a second `stopRecording()` would race the first and error out.
	if (isStopping) return;
	clearTimeout(saveResetTimer);
	try {
		isStopping = true;
		saveState = "saving";
		saveError = null;
		// Rust stop_recording drives the camera flush on every stop path, so the panel only requests the stop.
		await stopRecording();
		saveState = "saved";
		// Brief confirmation, then back to idle ready for the next take.
		saveResetTimer = setTimeout(() => (saveState = "idle"), 1400);
	} catch (e) {
		// Show it ON the panel: the old main-window toast landed behind whatever was being recorded.
		saveError = String(e);
		saveState = "failed";
	} finally {
		// Always reset: Rust `stop()` takes the session first, so a later failure still leaves it gone and Stop would error forever.
		recordingStartTime = null;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		emit("camera-recording-stopped");
		emit("refresh-recordings");
		micLevel = 0;
		// Back to idle, so the ResizeObserver and Tween effect expand the bar to the full control set.
		isRecording = false;
		isStopping = false;
	}
}

// Clear the post-stop confirmation/error back to the idle bar. The Rust save already ran; this only dismisses the panel state.
function dismissSave() {
	clearTimeout(saveResetTimer);
	saveState = "idle";
	saveError = null;
}

async function startActualRecording() {
	if (!selectedSource) {
		isStarting = false;
		return;
	}
	const options: RecordingOptions = {
		systemAudio: systemAudioOn,
		microphone: micOn,
		microphoneDeviceId: micOn ? selectedMicId : null,
		camera: cameraOn,
		// Rust feeds this to FFmpeg dshow as a friendly name, so pass the label, not the deviceId hash.
		cameraDeviceId: cameraOn ? selectedCameraName : null,
		// Read fresh at start (localStorage is shared across webviews); fps is capped to the monitor's refresh, the preference untouched.
		fps: clampFpsToDisplay(loadRecordingFps(), selectedSource),
		quality: loadRecordingQuality(),
	};
	// Roll the camera BEFORE the backend starts: start_recording blocks on thread spin-up, so a late start ran seconds behind.
	if (cameraOn) {
		emit("camera-recording-started", { startedAtUnixMs: Date.now() });
	}
	try {
		const result = await startRecording(
			selectedSource.type,
			selectedSource.id,
			options,
			selectedSource.type === "region" && selectedSource.region ? selectedSource.region : null,
		);
		// Flip both in one synchronous block so `phase` stays recording with no idle frame between.
		isRecording = true;
		isStarting = false;
		now = Date.now();
		recordingStartTime = now;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		// The ResizeObserver and Tween effect collapse the bar on the phase swap; nothing to do here.
		if (result.warnings.length > 0) {
			notify("warning", result.warnings.join("\n"), 8000);
		}
	} catch (e) {
		// Start failed: drop out of starting so the bar morphs back to idle instead of showing the transport.
		isStarting = false;
		notify("error", `Recording failed: ${e}`, 10000);
	}
}

async function togglePause() {
	if (!isRecording) return;
	try {
		if (isPaused) {
			await resumeRecording();
			if (cameraOn) void emit("camera-recording-resumed");
			if (pausedSince !== null) pausedAccumMs += Date.now() - pausedSince;
			pausedSince = null;
			isPaused = false;
		} else {
			await pauseRecording();
			if (cameraOn) void emit("camera-recording-paused");
			pausedSince = Date.now();
			isPaused = true;
		}
		// Rebuild the tray so its Pause/Resume label matches the new state.
		void refreshTray(true);
	} catch (e) {
		notify("error", `Pause/resume failed: ${e}`, 8000);
	}
}

// Asks the user to resume once a pause crosses 5 minutes, and every 5 after if dismissed. Never auto-stops.
$effect(() => {
	if (!isPaused || pausedSince === null) {
		lastPausePromptAt = null;
		return;
	}
	if (pausePromptOpen) return;
	const since = lastPausePromptAt ?? pausedSince;
	if (now - since >= PAUSE_PROMPT_INTERVAL_MS) {
		void promptPauseTimeout();
	}
});

async function promptPauseTimeout() {
	pausePromptOpen = true;
	try {
		const resume = await ask(
			"This recording has been paused for 5 minutes.\n\n" +
				"Resume now? (Use Stop on the panel to finish and save.)",
			{
				title: "Recast - recording paused",
				kind: "warning",
				okLabel: "Resume",
				cancelLabel: "Not now",
			},
		);
		if (resume && isPaused) {
			await togglePause();
		} else {
			// Stay paused, so re-arm so we prompt again in another 5 minutes.
			lastPausePromptAt = Date.now();
		}
	} catch {
		lastPausePromptAt = Date.now();
	} finally {
		pausePromptOpen = false;
	}
}

// Finalize first (trimming paused spans), then re-issue the close; `isClosing` lets the second pass through.
let isClosing = false;
async function finalizeAndClose() {
	isClosing = true;
	try {
		// Skip when a stop is already in flight (Stop button): a second stopRecording() races the first. Rust owns the session, so it still saves.
		if (isRecording && !isStopping) await stopRecording();
	} catch (e) {
		// Closing anyway would discard the take, so stay open and let the user retry the stop.
		console.error("finalize-on-close failed:", e);
		isClosing = false;
		notify(
			"error",
			`Couldn't finish the recording: ${e instanceof Error ? e.message : e}. The panel stayed open — try stopping again.`,
			8000,
		);
		return;
	}
	emit("refresh-recordings");
	closeCameraPreview();
	getCurrentWindow().close();
}

// Elapsed excludes paused time so the timer freezes while paused.
const elapsed = $derived.by(() => {
	if (!isRecording || recordingStartTime === null) return 0;
	const livePause = pausedSince !== null ? now - pausedSince : 0;
	const ms = now - recordingStartTime - pausedAccumMs - livePause;
	return Math.max(0, Math.floor(ms / 1000));
});
const timer = $derived(formatRecordingTimer(elapsed));

// Pin the leaving block absolute so it stops feeding the measured width, letting the bar tween while it crossfades.
function phaseOut(node: HTMLElement) {
	const w = node.offsetWidth;
	const h = node.offsetHeight;
	// Pinned centered, not top-left, so the leaving phase clips symmetrically instead of shifting left.
	node.style.position = "absolute";
	node.style.left = "50%";
	node.style.top = "50%";
	node.style.width = `${w}px`;
	node.style.height = `${h}px`;
	node.style.transform = "translate(-50%, -50%)";
	return {
		duration: 220,
		easing: cubicOut,
		css: (t: number) => `opacity: ${t}`,
	};
}
</script>

<div class="flex h-dvh w-dvw items-center justify-center px-4 py-3">
	<div
		class="group/panel relative flex h-11 shrink-0 items-center justify-center overflow-hidden no-scrollbar rounded-xl border border-border/60 bg-card/85 shadow-craft-floating ring-1 ring-inset ring-foreground/10 backdrop-blur-xl"
		style="width: {barWidth.current}px"
	>
		{#snippet CountDownPhase()}
			<button
				type="button"
				onclick={startNow}
				onmousedown={(e: MouseEvent) => e.stopPropagation()}
				title="Start now"
				aria-label={`Recording starts in ${countdownValue} seconds, click to start now`}
				class="group/cd relative flex size-7 shrink-0 items-center justify-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary/40"
			>
				<svg
					class="absolute inset-0 size-7 -rotate-90"
					viewBox="0 0 36 36"
					aria-hidden="true"
				>
					<circle
						cx="18"
						cy="18"
						r="16"
						fill="none"
						stroke="currentColor"
						stroke-width="3"
						class="text-primary/15"
					/>
					<circle
						cx="18"
						cy="18"
						r="16"
						fill="none"
						stroke="currentColor"
						stroke-width="3"
						stroke-linecap="round"
						class="text-primary"
						stroke-dasharray={RING_C}
						stroke-dashoffset={RING_C * (1 - countdownProgress)}
					/>
				</svg>
				<!-- On hover the second yields to a play glyph to reveal the skip affordance. -->
				{#key countdownValue}
					<span
						in:scale={{
							start: prefersReducedMotion ? 1 : 0.5,
							duration: prefersReducedMotion ? 0 : 220,
							easing: cubicOut,
						}}
						class="font-mono text-[13px] font-bold leading-none tabular-nums text-primary transition-opacity group-hover/cd:opacity-0"
					>
						{countdownValue}
					</span>
				{/key}
				<PlayFilled
					size={11}
					class="absolute text-primary opacity-0 transition-opacity group-hover/cd:opacity-100"
				/>
			</button>

			<span class="flex shrink-0 flex-col leading-tight">
				<span
					class="whitespace-nowrap text-[11px] font-semibold tracking-tight text-foreground"
				>
					Get ready…
				</span>
				<span
					class="whitespace-nowrap text-[10px] font-medium tabular-nums text-muted-foreground"
				>
					Starting in {countdownValue}s
				</span>
			</span>
		{/snippet}

		{#snippet RecordingPhase()}
			{#if micOn}
				<div
					class="flex h-4 shrink-0 items-end gap-0.5"
					title="Microphone input level"
					aria-hidden="true"
				>
					{#each METER_BARS as scale}
						<div
							class="h-full w-0.5 rounded-full bg-success motion-safe:transition-transform motion-safe:duration-75"
							style="transform: scaleY({Math.max(
								0.12,
								(isPaused ? 0 : micLevel) * scale,
							)}); transform-origin: bottom;"
						></div>
					{/each}
				</div>
			{:else}
				<LogoWave size="20" active={false} class="shrink-0" />
			{/if}

			<span
				class="relative ml-0.5 flex size-2 shrink-0"
				aria-hidden="true"
			>
				{#if !isPaused}
					<span
						class="absolute inline-flex size-full rounded-full bg-destructive opacity-60 motion-safe:animate-ping"
					></span>
				{/if}
				<span
					class="relative inline-flex size-2 rounded-full {isPaused
						? 'bg-muted-foreground'
						: 'bg-destructive'}"
				></span>
			</span>

			<span class="flex shrink-0 flex-col gap-0.5 leading-tight">
				<span
					class="text-[10px] font-bold uppercase tracking-widest {isPaused
						? 'text-muted-foreground'
						: 'text-destructive'}"
				>
					{isPaused ? "Paused" : "Recording"}
				</span>
				<span
					class="font-mono text-[15px] font-semibold leading-none tabular-nums tracking-tight text-foreground"
				>
					{timer}
				</span>
			</span>

			{#if cameraOn}
				<span
					class="shrink-0 border-l border-border/50 pl-2 text-muted-foreground"
					title="Camera on"
					aria-hidden="true"
				>
					<Camera size={12} stroke={2} />
				</span>
			{/if}

			<Button
				onclick={togglePause}
				onmousedown={(e: MouseEvent) => e.stopPropagation()}
				size="icon-sm"
				variant={isPaused ? "success_soft" : "secondary"}
				title={isPaused ? "Resume Recording" : "Pause Recording"}
			>
				{#if isPaused}
					<PlayFilled size={13} />
				{:else}
					<PauseFilled size={13} />
				{/if}
			</Button>
			<!-- Separated from Pause: an accidental Stop is a costly, irreversible mis-click mid-recording. -->
			<Button
				onclick={toggleRecording}
				onmousedown={(e: MouseEvent) => e.stopPropagation()}
				disabled={isStopping}
				size="icon-sm"
				variant="destructive_soft"
				title="Stop recording"
				aria-label="Stop recording"
			>
				<SquareFilled size={10} class="text-destructive" />
			</Button>
		{/snippet}
		{#snippet FinalizingPhase()}
			<div class="flex w-fit items-center gap-2 pl-0.5 pr-1" role="status" aria-live="polite">
				{#if saveState === "failed"}
					<span
						class="flex size-5 shrink-0 items-center justify-center rounded-full bg-destructive/10 text-destructive"
						aria-hidden="true"
					>
						<AlertTriangle size={12} stroke={2} />
					</span>
					<div class="flex shrink-0 flex-col leading-tight">
						<span class="whitespace-nowrap text-[11px] font-semibold tracking-tight text-foreground">
							Couldn't save
						</span>
						<span
							class="max-w-45 truncate text-[10px] font-medium text-muted-foreground"
							title={saveError ?? ""}
						>
							{saveError ?? "The recording may be incomplete."}
						</span>
					</div>
					<Button
						onclick={dismissSave}
						onmousedown={(e: MouseEvent) => e.stopPropagation()}
						size="xs"
						variant="outline"
						class="ml-1 shrink-0"
					>
						Dismiss
					</Button>
				{:else if saveState === "saved"}
					<span
						class="flex size-5 shrink-0 items-center justify-center rounded-full bg-success/10 text-success"
						aria-hidden="true"
					>
						<Check size={12} stroke={2.5} />
					</span>
					<span class="whitespace-nowrap text-[12px] font-semibold tracking-tight text-foreground">
						Saved
					</span>
				{:else}
					<LoaderCircle
						size={15}
						stroke={2}
						class="shrink-0 text-muted-foreground motion-safe:animate-spin"
					/>
					<span class="whitespace-nowrap text-[12px] font-medium tracking-tight text-muted-foreground">
						Saving recording…
					</span>
				{/if}
			</div>
		{/snippet}
		{#snippet IdlePhase()}
			<Button
				onclick={toggleRecording}
				onmousedown={(e: MouseEvent) => e.stopPropagation()}
				size="icon-sm"
				variant="default"
				title="Start Recording"
			>
				<RecordFilled size={14} />
			</Button>

			{#if !isRecording}
				<div class="inline-flex" out:fade={{ duration: 120 }}>
					<Button
						size="sm"
						disabled={isRecording}
						onclick={() => openSourceSelector()}
						onmousedown={(e: MouseEvent) => e.stopPropagation()}
						variant="ghost"
						class="group/source hover:scale-none"
					>
						{#if selectedSource?.type === "window"}
							<AppWindow
								size={12}
								stroke={2}
								class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
							/>
						{:else if selectedSource?.type === "region"}
							<Crop
								size={12}
								stroke={2}
								class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
							/>
						{:else}
							<Monitor
								size={12}
								stroke={2}
								class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
							/>
						{/if}
						<span
							class="max-w-35 truncate text-[12px] font-semibold tracking-tight text-foreground/60 group-hover/source:text-foreground/90 transition-colors"
						>
							{selectedSource?.label ?? "Select source"}
						</span>
						{#if !isRecording}
							<ChevronDown
								size={10}
								stroke={3}
								class="shrink-0 text-foreground/20 transition-transform group-hover/source:translate-y-0.5"
							/>
						{/if}
					</Button>
				</div>
			{/if}

			<!-- While recording, drop `ml-auto` so Close packs tight next to the transport. -->
			<div
				class="shrink-0 px-1 inline-flex items-center gap-1"
				class:ml-auto={!isRecording}
			>
				{#if !isRecording}
					<div
						class="inline-flex items-center gap-1"
						out:fade={{ duration: 120 }}
					>

						{#if profilesStore.enabled && profilesStore.profiles.length > 0}
							<Button
								size="icon-sm"
								variant={profileFlash
									? "default_soft"
									: "ghost"}
								disabled={isRecording}
								onclick={openProfilePicker}
								onmousedown={(e: MouseEvent) =>
									e.stopPropagation()}
								title={activeProfile
									? `Profile: ${activeProfile.name}. Click to switch.`
									: "Switch profile"}
								aria-label="Switch profile"
							>
								<SlidersIcon size={13} stroke={2} />
							</Button>
						{/if}

						{#if deviceIssue}
							<!-- Icon-only, not inline text: a long message would widen the bar past the window. The message rides the native title, which the OS draws outside the 72px window a hover card would be clipped by. -->
							<span
								class="flex size-6 shrink-0 items-center justify-center rounded-md {deviceIssue.level ===
								'error'
									? 'bg-destructive/10 text-destructive'
									: 'bg-warning/10 text-warning'}"
								role="img"
								aria-label={deviceIssue.text}
								title={deviceIssue.text}
							>
								<AlertTriangle size={12} stroke={2} />
							</span>
						{/if}

						<!-- Device toggles -->
						<ButtonGroup>
							<!-- System audio -->
							<Button
								size="icon-sm"
								variant={systemAudioOn
									? "active"
									: "outline"}
								disabled={isRecording}
								onclick={toggleSystemAudio}
								onmousedown={(e: MouseEvent) =>
									e.stopPropagation()}
								title={systemAudioOn
									? "System audio: on"
									: "System audio: off"}
							>
								{#if systemAudioOn}
									<Volume size={14} stroke={2} />
								{:else}
									<VolumeOff size={14} stroke={2} />
								{/if}
							</Button>

							<!-- micWarning (from applyProfile) surfaces in the tooltip, not a toast. -->
							<Button
								variant={micOn
									? micWarning
										? "destructive_soft"
										: "active"
									: micWarning
										? "destructive_soft"
										: "outline"}
								size="icon-sm"
								disabled={isRecording}
								onclick={toggleMic}
								onmousedown={(e: MouseEvent) =>
									e.stopPropagation()}
								title={micOn
									? `Mic: ${selectedMicName}${micWarning ? `. ${micWarning}` : ""}`
									: micWarning
										? `Microphone: off. ${micWarning}`
										: "Microphone: off"}
							>
								{#if micOn}
									<Mic size={14} stroke={2} />
								{:else}
									<MicOff size={14} stroke={2} />
								{/if}
							</Button>

					
							<Button
								disabled={isRecording}
								onclick={toggleCamera}
								onmousedown={(e: MouseEvent) =>
									e.stopPropagation()}
								variant={cameraOn
									? cameraValidation?.status === "error" ||
										cameraWarning
										? "destructive_soft"
										: "active"
									: cameraWarning
										? "destructive_soft"
										: "outline"}
								size="icon-sm"
								title={cameraOn
									? `Camera: ${selectedCameraName}${cameraValidation?.statusMessage ? `. ${cameraValidation.statusMessage}` : ""}${cameraWarning ? `. ${cameraWarning}` : ""}`
									: cameraWarning
										? `Camera: off. ${cameraWarning}`
										: "Camera: off"}
							>
								{#if cameraOn}
									<Camera size={14} stroke={2} />
								{:else}
									<CameraOff size={14} stroke={2} />
								{/if}
							</Button>
						</ButtonGroup>
					</div>
				{/if}
			</div>
		{/snippet}
		<div
			bind:this={barContentEl}
			data-tauri-drag-region
			class="relative flex w-fit shrink-0 items-center justify-center gap-1 p-2"
		>
			<div
				data-tauri-drag-region
				class="flex w-fit items-center gap-2 pl-0.5 pr-1"
				in:fade={{ duration: 200, delay: 80, easing: cubicOut }}
				out:phaseOut
			>
				<div
					data-tauri-drag-region
					class="flex h-7 w-5 shrink-0 cursor-grab items-center justify-center rounded text-muted-foreground/60 transition-colors hover:bg-muted/40 hover:text-muted-foreground active:cursor-grabbing"
					title="Drag to move"
					aria-label="Drag panel"
				>
					<GripVertical
						size={12}
						stroke={2}
						class="pointer-events-none"
					/>
				</div>
				{#if phase === "countdown"}
					{@render CountDownPhase()}
				{:else if phase === "recording"}
					{@render RecordingPhase()}
				{:else if phase === "finalizing"}
					{@render FinalizingPhase()}
				{:else}
					{@render IdlePhase()}
				{/if}
				{#if saveState !== "saving"}
					<!-- Close during recording is safe: onCloseRequested finalizes then closes. Only the in-flight save hides it, to keep the Saved confirmation on screen. -->
					<Button
						onclick={() =>
							phase === "countdown"
								? cancelCountdown()
								: phase === "finalizing"
									? dismissSave()
									: closePanel()}
						onmousedown={(e: MouseEvent) => e.stopPropagation()}
						title={phase === "recording"
							? "Stop and close"
							: phase === "finalizing"
								? "Dismiss"
								: "Close"}
						size="icon-sm"
						variant="ghost"
					>
						<X size={10} stroke={2} class="shrink-0 text-destructive" />
					</Button>
				{/if}
			</div>
		</div>
	</div>
</div>
