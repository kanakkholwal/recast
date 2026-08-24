<script lang="ts">
import { type BrowserCamera, enumerateCameras } from "@recast/editor/lib/camera/browser-devices";
import {
	loadRecordingFps,
	loadRecordingQuality,
	type RecordingProfile,
	resolveCamera,
	resolveMic,
} from "@recast/editor/lib/profiles";
import {
	AppWindow,
	Camera,
	CameraOff,
	ChevronDown,
	Crop,
	GripVertical,
	Mic,
	MicOff,
	Monitor,
	PauseFilled,
	PlayFilled,
	Record,
	RecordFilled,
	SlidersHorizontal as SlidersIcon,
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
import { page } from "$app/state";
import { checkCapability, loadCapabilities } from "$lib/capabilities";
import {
	type AudioDeviceInfo,
	CAPTURE_INTENT_CHANGED_EVENT,
	type CameraValidationResult,
	type CaptureIntentState,
	excludeWindowFromCapture,
	getAudioDevices,
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
	buildCaptureIntent,
	canonicalIntent,
	clampFpsToDisplay,
	deviceOutcome,
	formatRecordingTimer,
	intentToTargetType,
	lastSourceToTarget,
	sourceFromIntent,
	type TargetSource,
	targetToLastSource,
	targetTypeToIntent,
} from "./panel.logic";

// The panel is too small for its own Toaster, so emit `ui:toast` for the main
// window to render (see +layout.svelte). alert() is the fallback if emit throws.
type ToastLevel = "error" | "warning" | "info" | "success";
function notify(level: ToastLevel, message: string, duration?: number) {
	emit("ui:toast", { level, message, duration }).catch((err) => {
		console.error("ui:toast emit failed, falling back to alert", err);
		window.alert(message);
	});
}

let selectedSource: TargetSource | null = $state(null);
let isRecording = $state(false);
// True between the countdown ending and startRecording resolving. Treating it
// as "recording" stops `phase` dipping to "idle" and flashing the full bar.
let isStarting = $state(false);
// Guards against a second stop click while stop_recording is in flight (it can
// take a beat on macOS), which would race the backend and error spuriously.
let isStopping = $state(false);
let recordingStartTime: number | null = $state(null);
let now = $state(Date.now());

// `countdownValue` is the live integer tick (null unless counting down);
// `countdownProgress` (1 → 0) drives the depleting ring.
let countdownValue = $state<number | null>(null);
let countdownProgress = $state(1);
let countdownRaf: number | null = null;
// Ring circumference (r=16 in the 36×36 viewBox); dash offset = C × (1 − progress).
const RING_C = 2 * Math.PI * 16;

// The bar width tweens to follow its content's measured natural width. The
// Tauri window stays at its fixed launch size and is NOT resized per phase: a
// centered always-on-top window can't resize+reposition in one atomic frame,
// so the bar morphs centered inside it (transparent margins are a drag region).
const BAR_W_IDLE = 488;

let barContentEl = $state<HTMLElement | null>(null);
let measuredBarW = $state(BAR_W_IDLE);
const barWidth = new Tween(BAR_W_IDLE, { duration: 340, easing: cubicOut });
// Snap the very first measurement instead of animating from the seed value.
let barFirstMeasure = true;
const prefersReducedMotion =
	typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

// Linux (tao) doesn't honor always-on-top for these overlay windows, matching
// the guards in $lib/ipc for the panel + camera-preview windows.
const IS_LINUX = platform() === "linux";

// Tween the bar to the content's natural width. Border box so the bar wraps
// exactly; rounded to whole px so sub-pixel jitter can't retrigger.
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

	// Tween the bar to the new measured width (instant under reduced motion).
	// The window never moves; the bar morphs centered within it.
	if (prefersReducedMotion) barWidth.set(measuredBarW, { duration: 0 });
	else barWidth.target = measuredBarW;
});

// `idle` = full controls; `countdown` = number + cancel; `recording` = transport.
const phase = $derived<"idle" | "countdown" | "recording">(
	isRecording || isStarting ? "recording" : countdownValue !== null ? "countdown" : "idle",
);

// Mirror the recording flag to the tray label. Best-effort; no-op if tray
// init failed.
$effect(() => {
	void refreshTray(isRecording);
});

// `pausedAccumMs` banks completed pauses; `pausedSince` marks one in progress,
// and the elapsed timer subtracts both so it freezes.
let isPaused = $state(false);
let pausedAccumMs = $state(0);
let pausedSince: number | null = $state(null);

// While paused, re-prompt every 5 minutes, because the camera keeps recording
// through a pause, so a forgotten pause quietly wastes disk.
const PAUSE_PROMPT_INTERVAL_MS = 5 * 60 * 1000;
let pausePromptOpen = $state(false);
let lastPausePromptAt: number | null = $state(null);

// Device toggles
let systemAudioOn = $state(true);
let micOn = $state(false);
let cameraOn = $state(false);

// Selected devices
let selectedMicId = $state<string | null>(null);
let selectedMicName = $state("Default");
let selectedCameraId = $state<string | null>(null);
let selectedCameraName = $state("Default");
let cameraValidation = $state<CameraValidationResult | null>(null);

// Resolution outcomes (fallback / missing) surface in button tooltips since
// the panel can't host a toast; persist until the next apply or manual toggle.
let micWarning = $state<string | null>(null);
let cameraWarning = $state<string | null>(null);

// Available device lists, refreshed each time we resolve a profile so the
// resolver works against current hardware (USB devices come and go).
let mics = $state<AudioDeviceInfo[]>([]);
let cameras = $state<BrowserCamera[]>([]);

// Which profile is currently driving the panel state, if any. Manual toggle
// overrides don't clear this; the chip is just a "last applied" marker.
let activeProfileId = $state<string | null>(null);
// Briefly highlights the profile-switcher button after a successful apply
// so the user gets a confirmation cue without us popping a toast.
let profileFlash = $state(false);
let profileFlashTimer: ReturnType<typeof setTimeout> | null = null;

const activeProfile = $derived(activeProfileId ? profilesStore.findById(activeProfileId) : null);

// Active profile's override (0 = off, number = pinned, null = inherit) wins
// over the global setting. Derived off `activeProfile`, not snapshotted, so a
// live cross-window edit updates the countdown immediately.
const countdownSeconds = $derived(activeProfile?.countdown ?? recordingCountdown.value);

// --- Backend capture-intent sync (Phase 3b) -----------------------------
// The panel and the backend `CaptureIntent` stay in sync both ways: the panel
// pushes its selection so `recast selection show` reflects the live UI, and it
// applies external edits (from `recast select`/`set`) that arrive as
// `capture-intent:changed`. `lastIntent` is the last value we sent or received;
// comparing before writing/applying breaks the echo loop between the two.
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
				.catch(() => {});
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

	// Camera: the intent carries the DirectShow name; match a browser device by
	// label to drive the preview window.
	if (intent.options.camera) {
		const name = intent.options.cameraDeviceId ?? selectedCameraName;
		cameraOn = true;
		selectedCameraName = name;
		const match = cameras.find((c) => c.label === name);
		if (match) {
			selectedCameraId = match.deviceId;
			void refreshCameraValidation(match.deviceId);
			openCameraPreview(match.deviceId);
		}
		cameraWarning = null;
	} else if (cameraOn) {
		cameraOn = false;
		cameraValidation = null;
		closeCameraPreview();
	}
}

// On mount (after devices load): adopt a source the CLI staged before the
// panel opened, then seed the guard so the first push does not clobber it.
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

// Push the panel selection to the backend whenever it changes, unless it
// already matches (our own echo, or a value we just applied).
$effect(() => {
	const next = buildIntentFromPanel();
	if (!intentSyncReady) return;
	// Canonical compare: the backend echo omits null fields we send explicitly,
	// so a raw JSON compare never matches and would loop forever (panel froze).
	if (canonicalIntent(next) === canonicalIntent(lastIntent)) return;
	lastIntent = next;
	setCaptureIntent(next).catch(() => {});
});

async function refreshCameraValidation(deviceId: string | null) {
	if (!deviceId) {
		cameraValidation = null;
		return;
	}

	// Skip browser MediaDevices ids (hex hashes); the Rust validator only
	// knows DirectShow names; openCameraStream is the source of truth there.
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
		setLastSource(targetToLastSource(event.payload)).catch(() => {});
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
				openCameraPreview(id);
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

	// Prefer the last-used source from persisted config; fall back to the
	// primary display if no last source is recorded.
	getLastSource()
		.then((last) => {
			if (last) {
				selectedSource = lastSourceToTarget(last);
				// Look up the restored monitor's refresh rate so fps clamping knows
				// the display ceiling without a capture-time probe.
				if (selectedSource?.type === "monitor") {
					const restoredId = selectedSource.id;
					getDisplays()
						.then((displays) => {
							const hz = displays.find((d) => d.id === restoredId)?.refreshHz;
							if (hz && selectedSource && selectedSource.id === restoredId) {
								selectedSource = { ...selectedSource, refreshHz: hz };
							}
						})
						.catch(() => {});
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
		.catch(() => {})
		// Apply a launch intent (home mode tiles) only after the last source has
		// been restored, so "screen" wins over the restored source and the picker
		// modes open on top of a sensible base.
		.finally(() => void applyCaptureIntent(page.url.searchParams.get("intent")));

	// The panel may already be open when a mode tile is clicked; the intent
	// then arrives as an event instead of a URL param.
	const unlistenIntent = listen<{ intent: string }>(
		"panel-capture-intent",
		(event) => void applyCaptureIntent(event.payload.intent),
	);

	// Keep the camera toggle honest: if the preview window is closed by any
	// means (its own button, an OS close), reflect that instead of leaving the
	// button lit. Camera changes are locked during recording, so this only
	// applies before/after a take.
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
			// Canonical compare (see the push effect): ignore our own echo, whose
			// shape differs only by omitted-null fields, so we don't re-apply + loop.
			if (canonicalIntent(incoming) === canonicalIntent(lastIntent)) return;
			lastIntent = incoming;
			applyIntentToPanel(incoming);
		},
	);

	// Reflect a recording the panel did NOT start (CLI `rec start`, or a
	// `--timeout` auto-stop) in the transport. Panel-initiated takes set these
	// flags first, so the guard makes this a no-op for them.
	const unlistenRecStarted = listen<{ startedAtUnixMs: number }>("recording:started", (event) => {
		if (isRecording || isStarting) return;
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
		isRecording = false;
		isStopping = false;
		closeCameraPreview();
		emit("refresh-recordings");
	});
	// Non-fatal capture issues from stop (mic/camera failed to record, macOS
	// permission denied). The recording saved; surface these so the missing
	// track isn't a silent surprise in the editor.
	const unlistenRecWarnings = listen<string[]>("recording:warnings", (event) => {
		const messages = event.payload ?? [];
		if (messages.length > 0) {
			notify("warning", messages.join("\n"), 8000);
		}
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

	// Global hotkey (Alt+Shift+P) pauses/resumes while recording. Rust only
	// emits this when a recording is active, so the panel is the right owner.
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
		window.removeEventListener("keydown", handleGlobalShortcut);
	};
});

// Load devices, then apply the default profile if enabled; otherwise seed
// defaults (default mic, first non-virtual camera, only system audio on).
async function initDevicesAndProfile() {
	const [audioDevices, videoDevices] = await Promise.all([
		getAudioDevices().catch(() => [] as AudioDeviceInfo[]),
		enumerateCameras().catch(() => [] as BrowserCamera[]),
	]);
	mics = audioDevices;
	cameras = videoDevices;

	// Seed defaults even when applying a profile, so a later manual toggle has
	// something to use.
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

	// Profiles load from the backend now (async), so wait for them before
	// applying the default; the seeded device defaults above stand until then.
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
		openCameraPreview(camera.device.deviceId);
	} else {
		cameraValidation = null;
		// `missing` tears down unconditionally: the profile asked for a camera, so
		// say so even if nothing was up. `none` only closes what was open.
		if (camera.kind === "missing" || wasCameraOn) closeCameraPreview();
	}

	// The countdown follows the active profile live via the `countdownSeconds`
	// derived, so setting `activeProfileId` is all that's needed; no snapshot.
	activeProfileId = profile.id;
}

function handleProfileSwitch(profile: RecordingProfile) {
	if (isRecording) return;
	applyProfile(profile);
	// Brief 1.4s highlight on the profile button so the user gets a
	// visual confirmation without a toast.
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

function openSourceSelector(tab?: "monitor" | "window" | "region", autostart = false) {
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

// Apply a capture intent handed over from the home mode tiles (via the panel
// URL on first launch, or the "panel-capture-intent" event when already open).
// Additive only: never touches an in-progress recording.
async function applyCaptureIntent(intent: string | null | undefined) {
	if (!intent || isRecording) return;
	switch (intent) {
		case "screen": {
			// Full screen is unambiguous, so pick the primary display directly
			// rather than making the user confirm in the source picker.
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
			// No webcam-only source exists; add the camera overlay to the current
			// (screen) source. toggleCamera opens the camera picker when it's off.
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

function openCameraPreview(deviceId: string) {
	WebviewWindow.getByLabel("camera-preview").then(async (existing) => {
		if (existing) {
			await existing.close();
		}
		const win = new WebviewWindow("camera-preview", {
			url: `/camera-preview?deviceId=${encodeURIComponent(deviceId)}`,
			title: "Camera",
			width: 200,
			height: 200,
			decorations: false,
			transparent: true,
			shadow: false,
			alwaysOnTop: !IS_LINUX,
			resizable: true,
			skipTaskbar: true,
			x: 40,
			y: 40,
		});
		// MUST exclude from screen capture or DXGI Desktop Duplication bakes the
		// camera bubble into the recorded screen video. The HWND isn't reachable
		// until the window exists, so wait for `tauri://created`.
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
	// Enter "starting" before clearing the countdown so `phase` jumps straight
	// to "recording" rather than dipping through "idle" while the IPC resolves.
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
			// Bridge to "recording" via `isStarting` so the phase never falls back
			// to "idle" during the start IPC (see `isStarting` declaration).
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
	// While counting down, the Record button isn't shown, but a tray toggle
	// or shortcut could still land here; treat it as "cancel the countdown".
	if (countdownValue !== null) {
		cancelCountdown();
		return;
	}
	// Mid-handoff (countdown ended, start IPC in flight): ignore the toggle so
	// a stray click on the transitioning transport doesn't kick off a fresh
	// countdown before `isRecording` flips.
	if (isStarting) return;
	if (!isRecording) {
		beginRecording();
		return;
	}
	// A stop is already in flight, so ignore repeat clicks so we don't fire a
	// second `stopRecording()` that races the first and errors out.
	if (isStopping) return;
	try {
		isStopping = true;
		// The camera flush is driven by Rust stop_recording (covers every stop
		// path), so the panel just requests the stop.
		await stopRecording();
	} catch (e) {
		// Show the actual error, not a misleading "ffmpeg not installed"
		// suffix. By the time stop runs, start has already succeeded, so
		// FFmpeg was available, and a stop failure is something else
		// (encoder thread panic, disk full, codec mismatch in the
		// bundled binary, etc.). Misattributing to FFmpeg sent users
		// chasing missing-binary red herrings on bundles where FFmpeg
		// was actually present.
		notify("error", `Stop failed: ${e}`, 10000);
	} finally {
		// ALWAYS reset client-side state, even on stop failure. The Rust
		// `RecordingManager::stop()` does `guard.take()` as its first
		// operation, so once that succeeds, the session is gone from the
		// manager regardless of what later fails. Leaving `isRecording`
		// stuck at `true` traps the user into clicking Stop again, which
		// then errors with "recording is not running" because the session
		// is already gone. Resetting here lets the user start a new
		// recording immediately.
		recordingStartTime = null;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		emit("camera-recording-stopped");
		emit("refresh-recordings");
		// Back to "idle" phase, so the ResizeObserver → Tween effect expands the
		// bar back out to the full control set (centered in the fixed window).
		isRecording = false;
		isStopping = false;
	}
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
		// Rust feeds this directly to FFmpeg dshow as a DirectShow friendly
		// name, so pass the label, not the browser deviceId hash.
		cameraDeviceId: cameraOn ? selectedCameraName : null,
		// Global capture preferences set in Settings → Recording, read fresh at
		// start time (localStorage is shared across the app's webviews). The
		// backend clamps/validates both, so a stale or missing value is safe.
		// The desired fps is additionally capped to the selected monitor's
		// refresh, because capturing above it only duplicates frames, so e.g. a 144 fps
		// preference records at 60 on a 60 Hz display while still recording 144
		// on a 144 Hz one. The user's preference itself is left untouched.
		fps: clampFpsToDisplay(loadRecordingFps(), selectedSource),
		quality: loadRecordingQuality(),
	};
	// Roll the camera BEFORE the backend starts, not after: start_recording
	// blocks while capture/encoder/audio threads spin up, so emitting afterwards
	// left the camera seconds behind the screen. Starting early makes the offset
	// negative (extra head footage), which the session measures and trims.
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
		// Flip both in the same synchronous block: `phase` stays "recording"
		// (isStarting → isRecording) with no idle frame in between.
		isRecording = true;
		isStarting = false;
		now = Date.now();
		recordingStartTime = now;
		isPaused = false;
		pausedAccumMs = 0;
		pausedSince = null;
		// Flipping to the "recording" phase swaps in the compact transport; the
		// ResizeObserver → Tween effect collapses the bar (centered in the fixed
		// window) automatically. Nothing to do here.
		if (result.warnings.length > 0) {
			notify("warning", result.warnings.join("\n"), 8000);
		}
	} catch (e) {
		// Start failed, so drop out of "starting" so the bar morphs back to idle
		// instead of being stuck showing the recording transport.
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

// Pause-timeout nudge: once a pause crosses 5 minutes (and every 5 min
// after, if dismissed) ask the user to resume. Never auto-stops.
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

// Closing the panel mid-recording must not lose the take: finalize first
// (which trims out any paused spans), then re-issue the close. The
// `isClosing` guard lets that second close pass straight through.
let isClosing = false;
async function finalizeAndClose() {
	isClosing = true;
	try {
		if (isRecording) await stopRecording();
	} catch (e) {
		// Closing anyway would discard the take with no way back. Stay open so
		// the user can retry the stop.
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

// Out-transition for a leaving phase block: pin it absolute at its current
// size so it no longer contributes to the content's measured width while it
// fades. The entering block sits in normal flow, so ResizeObserver reports
// the *new* width immediately and the bar Tween animates to it concurrent
// with the crossfade. Same technique the export flow uses to swap phases.
function phaseOut(node: HTMLElement) {
	const w = node.offsetWidth;
	const h = node.offsetHeight;
	// Pin centered (not top-left) so that as the bar morphs to the smaller
	// incoming phase, the leaving phase clips/fades symmetrically from the
	// center instead of appearing to shift to the left.
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

<!-- Padding gives the panel's drop-shadow room; the window is transparent so
     it shows the desktop through. -->
<div class="flex h-dvh w-dvw items-center justify-center px-4 py-3">
  <div
    class="group/panel relative flex h-11 shrink-0 items-center justify-center overflow-hidden no-scrollbar bg-card/95 backdrop-blur-xl border border-border/60 rounded-lg ring-1 ring-foreground/5"
    style="width: {barWidth.current}px"
  >
    <!-- Content is `w-fit`; the bar tweens to follow it, centered, so collapse
       and expand are one symmetric motion. -->
    <div
      bind:this={barContentEl}
      class="relative flex w-fit shrink-0 items-center justify-center gap-1 p-2"
    >
      {#if phase === "countdown"}
        <!-- Depleting ring with the ticking second (click to start now), status, Cancel. -->
        <div
          class="flex w-fit items-center gap-2.5 pl-1"
          in:fade={{ duration: 200, delay: 80, easing: cubicOut }}
          out:phaseOut
        >
          <!-- The whole disc is a "start now" affordance. -->
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
                class="font-mono text-[12px] font-bold leading-none tabular-nums text-primary transition-opacity group-hover/cd:opacity-0"
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

          <Button
            onclick={cancelCountdown}
            onmousedown={(e: MouseEvent) => e.stopPropagation()}
            size="icon-sm"
            variant="ghost"
            title="Cancel (Esc)"
            aria-label="Cancel countdown"
          >
            <X size={12} stroke={2.5} class="text-destructive" />
          </Button>
        </div>
      {:else if phase === "recording"}
        <!-- Compact transport: Stop+timer, Pause, Close. -->
        <div
          class="flex w-fit items-center gap-1"
          in:fade={{ duration: 200, delay: 80, easing: cubicOut }}
          out:phaseOut
        >
          <ButtonGroup>
            <Button
              onclick={toggleRecording}
              onmousedown={(e: MouseEvent) => e.stopPropagation()}
              disabled={isStopping}
              size="sm"
              variant="destructive_soft"
              title="Stop Recording"
            >
              <Record size={11} class="animate-pulse text-destructive" />
              <span
                class="shrink-0 font-mono text-[13px] font-semibold tabular-nums tracking-tight"
                class:text-foreground={!isPaused}
                class:text-muted-foreground={isPaused}
              >
                {timer}
              </span>
            </Button>
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
          </ButtonGroup>
          <Button
            onclick={closePanel}
            onmousedown={(e: MouseEvent) => e.stopPropagation()}
            title="Close"
            size="icon-sm"
            variant="ghost"
          >
            <X size={10} stroke={2} class="shrink-0 text-destructive" />
          </Button>
        </div>
      {:else}
        <!-- Idle phase: full control set. -->
        <div
          class="flex w-fit items-center gap-1"
          in:fade={{ duration: 200, delay: 80, easing: cubicOut }}
          out:phaseOut
        >
          <!-- The whole bar is a drag region; the grip makes that discoverable. -->
          <div
            data-tauri-drag-region
            class="flex h-7 w-4 shrink-0 cursor-grab items-center justify-center rounded text-muted-foreground/40 transition-colors hover:bg-muted/40 hover:text-muted-foreground active:cursor-grabbing"
            title="Drag to move"
            aria-label="Drag panel"
          >
            <GripVertical size={12} stroke={2} class="pointer-events-none" />
          </div>
          <!-- Begins the countdown, or captures immediately when countdown is off. -->
          <Button
            onclick={toggleRecording}
            onmousedown={(e: MouseEvent) => e.stopPropagation()}
            size="icon-sm"
            variant="default"
            title="Start Recording"
          >
            <RecordFilled size={14} />
          </Button>

          <!-- Hidden once recording starts (source is locked in). Fade is on a wrapping
       div since Svelte transitions can't bind to a component. -->
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
                <!-- Opens a separate window, not a popover, because the panel is too short to host
         an in-place dropdown without resizing. -->
                {#if profilesStore.enabled && profilesStore.profiles.length > 0}
                  <Button
                    size="icon-sm"
                    variant={profileFlash ? "default_soft" : "ghost"}
                    disabled={isRecording}
                    onclick={openProfilePicker}
                    onmousedown={(e: MouseEvent) => e.stopPropagation()}
                    title={activeProfile
                      ? `Profile: ${activeProfile.name}. Click to switch.`
                      : "Switch profile"}
                    aria-label="Switch profile"
                  >
                    <SlidersIcon size={13} stroke={2} />
                  </Button>
                {/if}

                <!-- Device toggles -->
                <ButtonGroup>
                  <!-- System audio -->
                  <Button
                    size="icon-sm"
                    variant={systemAudioOn ? "default_soft" : "outline"}
                    disabled={isRecording}
                    onclick={toggleSystemAudio}
                    onmousedown={(e: MouseEvent) => e.stopPropagation()}
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
                        : "default_soft"
                      : micWarning
                        ? "destructive_soft"
                        : "outline"}
                    size="icon-sm"
                    disabled={isRecording}
                    onclick={toggleMic}
                    onmousedown={(e: MouseEvent) => e.stopPropagation()}
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

                  <!-- cameraWarning (profile apply) and cameraValidation (device probe) both
           surface in the tooltip; either wins the destructive_soft tint. -->
                  <Button
                    disabled={isRecording}
                    onclick={toggleCamera}
                    onmousedown={(e: MouseEvent) => e.stopPropagation()}
                    variant={cameraOn
                      ? cameraValidation?.status === "error" || cameraWarning
                        ? "destructive_soft"
                        : "default_soft"
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
            <!-- Close -->
            <Button
              onclick={closePanel}
              onmousedown={(e: MouseEvent) => e.stopPropagation()}
              title="Close"
              size="icon-sm"
              variant="ghost"
            >
              <X size={10} stroke={2} class="shrink-0 text-destructive" />
            </Button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
