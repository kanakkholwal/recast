<script lang="ts">
import { agentSession, Editor, resolveTrackOffsets } from "@recast/editor";
import AgentSessionBadge from "@recast/editor/components/AgentSessionBadge.svelte";
import BranchReviewPanel from "@recast/editor/components/BranchReviewPanel.svelte";
import ConfirmDialog from "@recast/editor/components/dialog/ConfirmDialog.svelte";
import EditorToolbar from "@recast/editor/components/EditorToolbar.svelte";
import ExportDialog from "@recast/editor/components/ExportDialog.svelte";
import ExportPanel, { type ExportPanelPhase } from "@recast/editor/components/ExportPanel.svelte";
import ExportStageLoader from "@recast/editor/components/ExportStageLoader.svelte";
import { DEFAULT_LAYOUT, LAYOUT_KEY, parseLayout } from "@recast/editor/editor-shell.logic";
import { clipAssetPath } from "@recast/editor/lib/audio/music";
import { activatesOnSpace, isOverlayOpen } from "@recast/editor/lib/dom/keyboard";
import {
	boolParam,
	PANEL_PARAM,
	parseBoolParam,
	parsePanelTab,
	SIDEBAR_PARAM,
	TIMELINE_PARAM,
	withEditorParams,
} from "@recast/editor/lib/editor/editor-url";
import { setEditorServices } from "@recast/editor/lib/editor/services";
import { formatClock, frameStepOutput } from "@recast/editor/lib/editor/time";
import {
	browserExportBlockedReason,
	resolveExportFps,
} from "@recast/editor/lib/export/browser-export-eligibility";
import type { ExportQuality } from "@recast/editor/lib/export/browser-export-plan";
import { buildExportJob } from "@recast/editor/lib/export/build-export-job";
import { chooseExportEngine } from "@recast/editor/lib/export/choose-export-engine";
import { probeBrowserExportCapability } from "@recast/editor/lib/export/export-capability";
import { exportEtaMs as computeExportEtaMs, formatElapsed } from "@recast/editor/lib/format/time";
import { AudioTimelineEngine, type MusicClipSpec } from "@recast/editor/lib/playback/audio-engine";
import { reconcileAvDrift } from "@recast/editor/lib/playback/av-drift";
import { decoderBudget } from "@recast/editor/lib/playback/decoder-budget";
import {
	buildCaptionExport,
	buildCloudCaptionTranscript,
	buildExportRenderState,
	exportTimeMap,
	findMissingImageAnnotations,
	hasBlurUnderZoom,
} from "@recast/editor/lib/services/export";
import {
	createTileProvider,
	type TileProvider,
} from "@recast/editor/lib/timeline/filmstrip-source";
import {
	originalToOutput,
	outputToOriginal,
	toRegions,
} from "@recast/editor/lib/timeline/time-map";
import type { CameraCapture } from "@recast/editor/lib/wire-types";
import { createEditorStore, type VideoMetadata } from "@recast/editor/stores/editor-store.svelte";
import { experimentalStore } from "@recast/editor/stores/experimental.svelte";
import type { IconComponent } from "@recast/icons";
import {
	ArrowLeft,
	BrandGoogleDrive,
	CheckCircle2,
	Clock,
	Copy,
	FlaskConical,
	FolderOpen,
	Play,
	TriangleAlert,
	Upload,
	VolumeX,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { Spinner } from "@recast/ui/spinner";
import { convertFileSrc } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { onDestroy, onMount, tick, untrack } from "svelte";
import { fade } from "svelte/transition";
import { browser } from "$app/environment";
import { afterNavigate, goto, replaceState } from "$app/navigation";
import { page } from "$app/state";
import UploadDialogsHost from "$components/cloud/UploadDialogsHost.svelte";
import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
import PlayerDialog from "$components/recast/PlayerDialog.svelte";
import RecastMark from "$components/recast-mark.svelte";
import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
import { type DestinationTile, destinationTile, uploadForPath } from "$lib/cloud/destination-tile";
import { acquireEditorWrite, releaseEditorWrite } from "$lib/editor/agent-session.tauri";
import { tauriEditorServices } from "$lib/editor/services.tauri";
import type { RecordingEntry } from "$lib/ipc";
import {
	autosaveProject,
	clearAutosave,
	createExportId,
	detectSilence,
	extractWaveform,
	generateThumbnails,
	getVideoMetadata,
	listExports,
	loadEditorDocument,
	migrateProject,
	openFileLocation,
	saveProjectEdits,
} from "$lib/ipc";
import { log } from "$lib/logger";
import { generateAutoZoom } from "$lib/services/analysis";
import { isShareSupported, shareRecording } from "$lib/share";
import { shareTargetFor } from "$lib/share-target";
import { registerShortcutHandlers } from "$lib/shortcuts/registry.svelte";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import { type ExportTelemetry, exportActivity } from "$lib/stores/exportActivity.svelte";
import { gdrive } from "$lib/stores/gdrive.svelte";
import { settingsHref } from "../../(app)/settings/settings-tabs";
import { basename } from "./editor-page.logic";

interface Props {
	data: {
		filePath: string;
		filename: string;
	};
}

let { data }: Props = $props();

// Context copy of the app-scoped services, matching how @recast/editor will read them.
setEditorServices(tauriEditorServices);

const store = createEditorStore();

let videoEl: HTMLVideoElement | null = $state(null);
// Engine drives the clock; echoing videoEl.currentTime here snaps playback back across cuts.
let webcodecsActive = $state(false);
// WYSIWYG screenshot (composite, not raw frame); bound from VideoPreview.
let captureFrame = $state<(() => Promise<Blob | null>) | undefined>(undefined);
// Loop-within-trim: here so `ended` and `timeupdate` share one pause-vs-loop decision.
let loopEnabled = $state(false);

// Persisted sidebar/timeline visibility; missing or malformed falls back to all visible.
function loadLayout(): { sidebar: boolean; timeline: boolean } {
	if (!browser) return { ...DEFAULT_LAYOUT };
	return parseLayout(localStorage.getItem(LAYOUT_KEY));
}
const initialLayout = loadLayout();
let showSidebar = $state(initialLayout.sidebar);
let showTimeline = $state(initialLayout.timeline);

$effect(() => {
	if (!browser) return;
	try {
		localStorage.setItem(
			LAYOUT_KEY,
			JSON.stringify({ sidebar: showSidebar, timeline: showTimeline }),
		);
	} catch {
		// localStorage can throw in private mode; the toggle works, it just isn't remembered.
	}
});

// --- View state ⇄ URL: a URL param beats the remembered layout; reader effect first so deep links win. ---
$effect(() => {
	const params = page.url.searchParams;
	const tab = parsePanelTab(params.get(PANEL_PARAM), import.meta.env.DEV);
	const sidebar = parseBoolParam(params.get(SIDEBAR_PARAM));
	const timeline = parseBoolParam(params.get(TIMELINE_PARAM));
	untrack(() => {
		if (tab && tab !== store.activePanel) store.activePanel = tab;
		if (sidebar !== null && sidebar !== showSidebar) showSidebar = sidebar;
		if (timeline !== null && timeline !== showTimeline) showTimeline = timeline;
	});
});

// `replaceState` throws until the router boots; the first `afterNavigate` is the earliest safe point.
let routerReady = $state(false);
afterNavigate(() => {
	routerReady = true;
});

$effect(() => {
	// Read all three before the guard so this stays subscribed to every one.
	const next = {
		[PANEL_PARAM]: store.activePanel,
		[SIDEBAR_PARAM]: boolParam(showSidebar),
		[TIMELINE_PARAM]: boolParam(showTimeline),
	};
	if (!routerReady) return;
	const url = withEditorParams(
		untrack(() => new URL(page.url)),
		next,
	);
	if (!url) return;
	// replaceState, not goto: view state must not put a history entry behind every toggle.
	replaceState(
		url,
		untrack(() => page.state),
	);
});

let previewContainerEl: HTMLElement | null = $state(null);
let videoSrc = $state("");
let systemAudioSrc = $state("");
let micAudioSrc = $state("");
// Cut-aware audio for both preview paths; if init/decode fails the preview plays silent.
let audioEngine: AudioTimelineEngine | null = $state(null);
let audioEngineTried = false;
// Bumped on document change/destroy so an engine that finishes decoding later knows it is stale.
let audioEngineGen = 0;
let cursorPath = $state<string | null>(null);
let cameraPath = $state<string | null>(null);
// Separates camera-off from a project predating camera capture; the path alone cannot.
let cameraCapture = $state<CameraCapture>("legacy");
// Per-track capture lag; preview must apply the same shift as export or the two disagree.
let trackOffsets = $state(resolveTrackOffsets(undefined));
let cameraSrc = $state("");
let documentPath = $state("");
let isLoading = $state(true);
let error = $state("");
let loadedPath = $state("");
let thumbnailToken = 0;

// 48 = clip-bar h-12 in CSS px; a null tileProvider means the stretched Rust strip.
const FILMSTRIP_TILE_HEIGHT = 48;
let tileProvider = $state<TileProvider | null>(null);
let filmstripVersion = $state(0);
let tileProviderToken = 0;

// Preview owns decode priority: DecoderBudget pauses the filmstrip decoder while playing or scrubbing.
let unregisterFilmstripLease: (() => void) | null = null;
let scrubBusyTimer: ReturnType<typeof setTimeout> | undefined;
let lastPreviewTime = -1;
$effect(() => {
	const playing = store.isPlaying;
	const ct = store.currentTime;
	if (playing) {
		lastPreviewTime = ct;
		decoderBudget.setPreviewBusy(true);
		return;
	}
	if (ct !== lastPreviewTime) {
		lastPreviewTime = ct;
		decoderBudget.setPreviewBusy(true);
		clearTimeout(scrubBusyTimer);
		scrubBusyTimer = setTimeout(() => decoderBudget.setPreviewBusy(false), 300);
	} else {
		decoderBudget.setPreviewBusy(false);
	}
});

// A v1 `.recast` must migrate first; migrationDone separates a confirmed update (reload) from a dismissal (leave).
let showMigration = $state(false);
let migrationDone = false;

// Autosave: save edit state every 30 seconds while editing.
const AUTOSAVE_INTERVAL_MS = 30_000;
let autosaveTimer: ReturnType<typeof setInterval> | null = null;

// Sonner dedupes on id, so a retrying autosave shows one toast instead of a stream.
const AUTOSAVE_TOAST_ID = "autosave-failed";
let autosaveFailing = false;

function startAutosave() {
	stopAutosave();
	autosaveTimer = setInterval(async () => {
		if (!documentPath || isLoading) return;
		// Most idle ticks are clean, so skip the full serialize until there is real work to persist.
		if (!store.isDirty) return;
		try {
			const editsJson = JSON.stringify(store.toRenderState());
			await autosaveProject(documentPath, editsJson);
			if (autosaveFailing) {
				autosaveFailing = false;
				toast.dismiss(AUTOSAVE_TOAST_ID);
			}
		} catch (err) {
			// Autosave is all that protects 30s of edits, so a failure must not stay in the console.
			console.warn("Autosave failed:", err);
			autosaveFailing = true;
			toast.error("Autosave isn't working", {
				id: AUTOSAVE_TOAST_ID,
				description: `${(err as Error)?.message ?? err} — save manually to keep your edits.`,
				duration: Number.POSITIVE_INFINITY,
				action: { label: "Save now", onClick: () => void handleSave() },
			});
		}
	}, AUTOSAVE_INTERVAL_MS);
}

function stopAutosave() {
	if (autosaveTimer !== null) {
		clearInterval(autosaveTimer);
		autosaveTimer = null;
	}
}

// The GUI is a first-class lock holder, so an agent patching this open project is refused, not raced.
const editorWriterId = `ui:${crypto.randomUUID().slice(0, 8)}`;

/** Re-read the saved edits after a branch lands, so the editor shows what was
 *  actually written rather than the pre-apply state. */
async function reloadRenderStateFromDisk() {
	if (!documentPath) return;
	try {
		const document = await loadEditorDocument(documentPath);
		store.loadRenderState(document.renderState);
		store.markSaved(Date.now());
	} catch (err) {
		log.warn("editor", "reload after branch apply failed", { err: String(err) });
	}
}

$effect(() => {
	const path = documentPath;
	if (!path) return;
	let cancelled = false;

	acquireEditorWrite(path, editorWriterId).catch((err) => {
		// An agent already holds it: stay read-only; agentSession.active drives the banner and inert panels.
		if (!cancelled) log.warn("editor", "write-lock unavailable", { err: String(err) });
	});

	const unbind = agentSession.bind({
		store,
		projectPath: path,
		// Refuse-by-default: unsaved edits are never discarded without a choice.
		onConflict: () =>
			new Promise<boolean>((resolve) => {
				toast.warning("The agent changed this project", {
					description: "You have unsaved edits, so its version wasn't loaded.",
					duration: 15_000,
					action: { label: "Load agent version", onClick: () => resolve(true) },
					onDismiss: () => resolve(false),
					onAutoClose: () => resolve(false),
				});
			}),
	});

	return () => {
		cancelled = true;
		unbind();
		void releaseEditorWrite(editorWriterId).catch(() => undefined);
	};
});

// A fresh editor has no panel open yet, so clear stale foreground from another route's "Open export".
onMount(() => {
	exportActivity.setEditorPresent(true);
	exportActivity.minimize();
	// Re-adopt a running/queued export for this project so its panel survives navigating back.
	const mine = exportActivity.items.find(
		(i) => i.filePath === data.filePath && (i.status === "running" || i.status === "queued"),
	);
	if (mine) myExportId = mine.id;
	return () => exportActivity.setEditorPresent(false);
});

onDestroy(() => {
	stopAutosave();
	log.clearRecast();
	// Clear autosave on clean exit.
	if (documentPath) {
		clearAutosave(documentPath).catch(() => undefined);
	}
	// Keep a live export tracked in the activity center after navigation; only drop the foreground flag.
	exportActivity.minimize();
});

// Seeks video + audio to trimStart and resumes; returns true so the timeupdate handler can bail.
function loopBackToStart(): boolean {
	if (!videoEl) return false;
	const start = store.trimStart || 0;
	videoEl.currentTime = start;
	// WebCodecs path: the <video> stays paused by design, so play()ing it just rejects with AbortError.
	if (webcodecsActive) {
		store.currentTime = start;
		return true;
	}
	// play() can reject (user-gesture), so log instead of stalling silently.
	void videoEl.play().catch((err) => {
		console.warn("loop replay failed:", err);
	});
	store.isPlaying = true;
	return true;
}

function handleTimeUpdate() {
	if (!videoEl) return;
	if (store.isPlaying) {
		// Legacy <video> path only: in the WebCodecs path the clock owns time and audio.
		if (webcodecsActive) return;
		store.currentTime = videoEl.currentTime;
		// Loop only matters below the natural duration; the natural end uses `ended`, not this ~250ms tick.
		if (loopEnabled && store.metadata) {
			const trimEnd = store.trimEnd > 0 ? store.trimEnd : store.metadata.duration;
			if (
				trimEnd > 0 &&
				trimEnd < store.metadata.duration - 0.05 &&
				videoEl.currentTime >= trimEnd - 0.05
			) {
				loopBackToStart();
				return;
			}
		}
	}
}

// Returns true when we looped, so the WebCodecs caller keeps its clock running and skips the pause below.
function handleVideoEnded(): boolean {
	if (loopEnabled && videoEl) {
		return loopBackToStart();
	}
	store.isPlaying = false;
	audioEngine?.pause();
	return false;
}

// Drift before audio is nudged forward; audio ahead of a stalled picture is never rewound (that echoes).
const AUDIO_SYNC_THRESHOLD = 0.12;
// A jump this far past one publish quantum means a cut crossing or scrub, so audio snaps on cuts of any length.
const AUDIO_JUMP = 0.12;
// Audio lead over a stalled picture before the PICTURE is advanced instead; bounds lip-sync drift under load.
const AUDIO_MAX_LEAD = 0.5;
// A reschedule restarts every source node; correcting at rAF rate would stutter far worse than the drift.
const RESYNC_COOLDOWN_MS = 250;
let audioSyncRaf: number | null = null;
let lastAudioTarget = -1;
let lastResyncMs = 0;
/**
 * Legacy <video> path only. There the element is the master and ALREADY skips
 * cuts (it jumps to cut.end at each boundary), so the engine is reconciled onto
 * its clock. The WebCodecs path runs the other way — VideoPreview reads
 * `audioPositionSec` — and is handled by the reschedule effect below.
 */
function syncAudioToClock() {
	audioSyncRaf = requestAnimationFrame(syncAudioToClock);
	const eng = audioEngine;
	if (!store.isPlaying || !eng || !videoEl) {
		lastAudioTarget = -1;
		return;
	}
	// Track the picture even before the engine has an audible position, or the covered gap later reads as a jump.
	const pictureOut = originalToOutput(store.timeMap, videoEl.currentTime);
	const jumped = lastAudioTarget >= 0 && Math.abs(pictureOut - lastAudioTarget) > AUDIO_JUMP;
	lastAudioTarget = pictureOut;
	const audioOut = eng.positionOutputSec;
	if (audioOut === null) return;
	const action = reconcileAvDrift({
		audioTime: audioOut,
		pictureTime: pictureOut,
		isJump: jumped,
		syncThreshold: AUDIO_SYNC_THRESHOLD,
		maxLead: AUDIO_MAX_LEAD,
	});
	if (action === "resync-audio") {
		// A seek/cut snaps immediately; only slow drift waits out the cooldown.
		const now = performance.now();
		if (!jumped && now - lastResyncMs < RESYNC_COOLDOWN_MS) return;
		lastResyncMs = now;
		eng.reschedule(audioRegions(), pictureOut);
	} else if (action === "catch-picture") {
		videoEl.currentTime = outputToOriginal(store.timeMap, audioOut);
	}
}
function startAudioClockSync() {
	if (audioSyncRaf === null) audioSyncRaf = requestAnimationFrame(syncAudioToClock);
}
function stopAudioClockSync() {
	if (audioSyncRaf !== null) {
		cancelAnimationFrame(audioSyncRaf);
		audioSyncRaf = null;
	}
}
onDestroy(stopAudioClockSync);
onDestroy(() => {
	// Bump first: an engine still decoding would resolve into a destroyed component and never be disposed.
	audioEngineGen++;
	audioEngine?.dispose();
});
onDestroy(disposeTileProvider);

// `keptTimeMap`, never `timeMap`: the latter un-collapses to the whole recording mid trim-drag.
function audioRegions() {
	return toRegions(store.keptTimeMap);
}
function outputNow() {
	return originalToOutput(store.timeMap, store.currentTime);
}
// Tried once; on failure the preview plays silent rather than dragging a second audio path along.
async function ensureAudioEngine() {
	if (audioEngine || audioEngineTried) return;
	audioEngineTried = true;
	if (!systemAudioSrc && !micAudioSrc) return;
	const gen = audioEngineGen;
	try {
		const eng = await AudioTimelineEngine.create([
			{ src: systemAudioSrc, kind: "system", offsetSec: trackOffsets.audioMs / 1000 },
			{ src: micAudioSrc, kind: "mic", offsetSec: trackOffsets.microphoneMs / 1000 },
		]);
		// Decode takes seconds on a long recording; adopting a stale engine strands its AudioContext and both PCM buffers.
		if (gen !== audioEngineGen) {
			eng.dispose();
			return;
		}
		const s = store.audioSettings;
		// Detached: the recording's audio plays as voice clips, so the monolithic source tracks are muted.
		const detached = store.audioDetached;
		eng.setMasterVolume(s.volume, s.muted);
		eng.setTrackVolume("system", detached ? 0 : s.systemVolume, detached || s.systemMuted);
		eng.setTrackVolume("mic", detached ? 0 : s.micVolume, detached || s.micMuted);
		eng.setFades(s.fadeIn, s.fadeOut, store.timeMap.outputDuration);
		void eng.setMusicClips(buildMusicSpecs());
		audioEngine = eng;
	} catch (err) {
		console.warn("Web Audio engine unavailable; the preview will be silent:", err);
	}
}

// Lockstep with `isPlaying` on both paths; the rAF reconciler runs only on the legacy path, where the <video> is the clock.
$effect(() => {
	const playing = store.isPlaying;
	const wc = webcodecsActive;
	const eng = audioEngine;

	if (playing) {
		void ensureAudioEngine();
		// `outputNow()` reads store.currentTime, which the legacy path publishes at only 4 Hz; the element's time is current.
		const from = untrack(() =>
			wc || !videoEl ? outputNow() : originalToOutput(store.timeMap, videoEl.currentTime),
		);
		if (eng) {
			void eng.play(
				untrack(() => audioRegions()),
				from,
			);
			// Seed the reconciler at the position we started from; left at -1 the first frame reads as a jump.
			lastAudioTarget = from;
		}
	} else {
		eng?.pause();
	}

	if (playing && !wc) startAudioClockSync();
	else stopAudioClockSync();
});

// Output jump that counts as a seek/loop; crossing a cut doesn't move gapless OUTPUT time, so it never fires.
const ENGINE_RESEEK_JUMP = 0.15;
let engineSyncOut = -1;
let lastRegionsKey = "";
$effect(() => {
	const t = store.currentTime;
	const eng = audioEngine;
	if (!eng || !webcodecsActive || !store.isPlaying) {
		engineSyncOut = -1;
		lastRegionsKey = "";
		return;
	}
	const out = originalToOutput(store.timeMap, t);
	const regions = audioRegions();
	const regionsKey = regions
		.map((r) => `${r.start.toFixed(3)}-${r.end.toFixed(3)}@${r.speed.toFixed(3)}`)
		.join(",");
	const jumped = engineSyncOut >= 0 && Math.abs(out - engineSyncOut) > ENGINE_RESEEK_JUMP;
	const editsChanged = lastRegionsKey !== "" && regionsKey !== lastRegionsKey;
	engineSyncOut = out;
	lastRegionsKey = regionsKey;
	if (jumped || editsChanged) eng.reschedule(regions, out);
});

// Master is the product of the per-track gains, so mic and system audio still mute independently.
$effect(() => {
	const settings = store.audioSettings;
	// Detached audio: source tracks are silenced so the un-cut source can't double-play under the voice clips.
	const detached = store.audioDetached;
	audioEngine?.setMasterVolume(settings.volume, settings.muted);
	audioEngine?.setTrackVolume(
		"system",
		detached ? 0 : settings.systemVolume,
		detached || settings.systemMuted,
	);
	audioEngine?.setTrackVolume(
		"mic",
		detached ? 0 : settings.micVolume,
		detached || settings.micMuted,
	);
	// Re-arm fades on setting change and when cuts/speed reshape output length.
	audioEngine?.setFades(settings.fadeIn, settings.fadeOut, store.timeMap.outputDuration);
});

// Resolve the store's music clips to playable specs (asset URLs).
function buildMusicSpecs(): MusicClipSpec[] {
	return store.musicClips.map((c) => ({
		url: convertFileSrc(clipAssetPath(c.source)),
		startOutputSec: c.startOutputSec,
		offsetSec: c.offsetSec,
		durationSec: c.durationSec,
		gain: c.muted ? 0 : c.gain,
		fadeIn: c.fadeIn,
		fadeOut: c.fadeOut,
		loop: c.loop,
	}));
}

// Reads the clips reactively on every add/remove/edit; the engine dedupes decode work per call.
$effect(() => {
	const specs = buildMusicSpecs();
	void store.timeMap.outputDuration; // reschedule fill length on edit
	audioEngine?.setMusicClips(specs);
});

// External seeks move the <video> so both paths realign off it; setting `currentTime` alone loses to the next legacy time-publish.
$effect(() => {
	const off = store.registerSeekHandler((t) => {
		if (videoEl) videoEl.currentTime = t;
	});
	return off;
});

// Skipped on the WebCodecs path, where audio follows the clock and snapping to seeks would fight it.
function handleVideoSeeked() {
	if (!videoEl || webcodecsActive) return;
	const t = videoEl.currentTime;
	// Publish the jumped position now; captions/overlays keyed off `store.currentTime` otherwise lag a cut by ~250 ms.
	store.currentTime = t;
	// No resnap here: the rAF reconciler already treats this as a jump, and both would restart the graph twice.
}

// Steps on the OUTPUT axis so a step across a cut lands on the next kept frame, never inside a removed range.
function frameStepSeek(direction: 1 | -1) {
	if (!store.metadata) return;
	const orig = frameStepOutput(store.timeMap, store.metadata, store.currentTime, direction);
	if (videoEl) videoEl.currentTime = orig;
	store.currentTime = orig;
}

function mergeVideoMetadata(next: Partial<VideoMetadata>) {
	store.metadata = {
		duration: next.duration ?? store.metadata?.duration ?? 0,
		width: next.width ?? store.metadata?.width ?? 0,
		height: next.height ?? store.metadata?.height ?? 0,
		fps: next.fps ?? store.metadata?.fps ?? 30,
		codec: next.codec ?? store.metadata?.codec ?? "unknown",
		sizeBytes: next.sizeBytes ?? store.metadata?.sizeBytes ?? 0,
	};
	if (store.trimEnd <= 0 && store.metadata.duration > 0) {
		store.loadRenderState({ trimEnd: store.metadata.duration });
	}
}

function disposeTileProvider() {
	unregisterFilmstripLease?.();
	unregisterFilmstripLease = null;
	clearTimeout(scrubBusyTimer);
	tileProvider?.dispose();
	tileProvider = null;
}

// Tokened so a rapid reopen disposes a provider that resolves after we moved on.
async function setupTileProvider(url: string) {
	const token = ++tileProviderToken;
	disposeTileProvider();
	const dpr = browser ? window.devicePixelRatio || 1 : 1;
	const provider = await createTileProvider({
		src: url,
		sizeBytes: store.metadata?.sizeBytes,
		durationSec: store.metadata?.duration,
		tileHeightPx: Math.round(FILMSTRIP_TILE_HEIGHT * dpr),
		onChange: () => {
			filmstripVersion++;
		},
	});
	if (token !== tileProviderToken) {
		provider?.dispose();
		return;
	}
	tileProvider = provider;
	if (provider) {
		unregisterFilmstripLease = decoderBudget.registerSecondary({
			onPause: (paused) => provider.setDecodePaused(paused),
		});
	}
}

async function loadThumbnailStrip(path: string) {
	// Skip without a duration: bumping the token would cancel an in-flight strip, and 0-duration yields black frames.
	const duration = store.metadata?.duration ?? 0;
	if (duration <= 0) return;

	const token = ++thumbnailToken;
	try {
		const count = duration > 60 ? 12 : 8;
		const strip = await generateThumbnails(path, count);
		if (token === thumbnailToken) {
			store.thumbnailStrip = strip;
		}
	} catch (err) {
		console.error("Thumbnail generation failed", err);
		if (token === thumbnailToken) {
			store.thumbnailStrip = [];
		}
	}
}

// Latch so the lazy scheduler fires once per loaded clip (reset on load).
let waveformRequested = false;

// Decode the audio peak envelope for the timeline waveform. Best-effort async.
async function loadWaveform() {
	// Skip sub-5s clips: too narrow to read, and the FFmpeg pass isn't worth it.
	const duration = store.metadata?.duration ?? 0;
	if (duration > 0 && duration < 5) {
		store.waveform = [];
		return;
	}
	try {
		store.waveform = await extractWaveform(store.audioPath, store.microphonePath);
	} catch (err) {
		console.warn("Waveform extraction failed", err);
		store.waveform = [];
	}
	// Warms the silence cache off the waveform pass (result discarded here) so the review popover opens instantly.
	void warmSilenceCache();
}

async function warmSilenceCache() {
	try {
		await detectSilence(store.audioPath, store.microphonePath, store.cursorPath);
	} catch (err) {
		console.warn("Silence precompute failed", err);
	}
}

function handleVideoLoadedMetadata() {
	if (!videoEl) return;
	mergeVideoMetadata({
		duration: videoEl.duration,
		width: videoEl.videoWidth,
		height: videoEl.videoHeight,
	});
}

function handleVideoReady() {
	handleVideoLoadedMetadata();
	isLoading = false;
	startAutosave();
}

function handleVideoError() {
	const code = videoEl?.error?.code;
	error = code
		? `Failed to load source media (media error ${code}).`
		: "Failed to load source media.";
	isLoading = false;
}

// Defers heavy secondary work off the preview's cold start; fires at browser-idle or the timeout.
function runWhenIdle(fn: () => void, timeout = 2000) {
	if (typeof requestIdleCallback === "function") {
		requestIdleCallback(fn, { timeout });
	} else {
		setTimeout(fn, 300);
	}
}

async function loadDocument() {
	error = "";
	isLoading = true;
	videoSrc = "";
	systemAudioSrc = "";
	micAudioSrc = "";
	cursorPath = null;
	cameraPath = null;
	cameraCapture = "legacy";
	trackOffsets = resolveTrackOffsets(undefined);
	cameraSrc = "";
	videoEl?.pause();
	// The bump also disowns an engine still decoding, which `dispose()` alone cannot reach.
	audioEngineGen++;
	audioEngine?.dispose();
	audioEngine = null;
	audioEngineTried = false;
	store.metadata = null;
	store.reset();
	store.thumbnailStrip = [];
	disposeTileProvider();

	try {
		const document = await loadEditorDocument(data.filePath);
		if (document.needsMigration) {
			// Stop before loading anything, and prompt to update the format first.
			isLoading = false;
			showMigration = true;
			return;
		}
		documentPath = document.projectPath;
		store.videoPath = document.projectPath;
		store.metadata = document.metadata;
		store.loadRenderState(document.renderState);
		// Scope every subsequent log in this window to the opened recast.
		log.setRecast(documentPath, {
			width: document.metadata.width,
			height: document.metadata.height,
			durationSec: Math.round(document.metadata.duration),
			fps: document.metadata.fps,
			codec: document.metadata.codec,
		});
		videoSrc = convertFileSrc(document.mediaPath);
		cursorPath = document.cursorPath ?? null;
		store.cursorPath = cursorPath;
		// Raw on-disk media paths for Rust-side analysis (silence detection).
		store.recordingPath = document.mediaPath;
		store.audioPath = document.audioPath ?? null;
		store.microphonePath = document.microphonePath ?? null;
		// Captions ride the audio's wall-clock axis; count-based CFR makes it differ, so rescale onto frame time.
		store.captionAudioDurationSec = null;
		{
			const capAudioPath = document.microphonePath ?? document.audioPath;
			if (capAudioPath) {
				getVideoMetadata(capAudioPath)
					.then((m) => {
						store.captionAudioDurationSec = m.duration > 0 ? m.duration : null;
					})
					.catch(() => undefined);
			}
		}
		store.waveform = [];
		// Lazy: the idle effect below extracts the waveform, so the ffmpeg pass never competes with load.
		waveformRequested = false;
		systemAudioSrc = document.audioPath ? convertFileSrc(document.audioPath) : "";
		micAudioSrc = document.microphonePath ? convertFileSrc(document.microphonePath) : "";
		trackOffsets = resolveTrackOffsets(document.trackOffsets);
		cameraPath = document.cameraPath ?? null;
		// Absent from an older backend: unknowable, so `legacy` — never "off".
		cameraCapture = document.cameraCapture ?? "legacy";
		cameraSrc = cameraPath ? convertFileSrc(cameraPath) : "";
		// Mount the editor body (VideoPreview renders only when !isLoading) so the <video> exists before load().
		isLoading = false;
		await tick();
		videoEl?.load();
		// Defer the thumbnail strip, filmstrip decoder and auto-zoom pass to idle; on 4K all three decode the same file and spiked open.
		const openedPath = document.projectPath;
		const filmstripSrc = videoSrc;
		runWhenIdle(() => {
			if (documentPath !== openedPath) return;
			void loadThumbnailStrip(openedPath);
			void setupTileProvider(filmstripSrc);
			void maybeRunAutoZoom();
		});
	} catch (err) {
		console.error("Failed to load editor document", err);
		log.error("session", "recast_load_failed", { error: String(err) });
		error = `Could not load project: ${err}`;
		isLoading = false;
	}
}

// Throwing here keeps ConfirmDialog open with the error shown.
async function confirmMigration() {
	await migrateProject(data.filePath);
	migrationDone = true;
}

function onMigrationOpenChange(open: boolean) {
	if (open) return;
	if (migrationDone) {
		migrationDone = false;
		void loadDocument();
	} else {
		void goto("/recasts");
	}
}

// `autoZoomApplied` stops a reopen from repopulating regions the user cleared.
let autoZoomRunning = false;

async function maybeRunAutoZoom() {
	if (autoZoomRunning) return;
	if (!store.autoZoomEnabled || store.autoZoomApplied) return;
	if (!cursorPath) {
		// No cursor track to analyse, so latch the flag so we don't retry on reopen.
		store.autoZoomApplied = true;
		return;
	}
	if (store.zoomRegions.length > 0) {
		// Regions already exist (autosave-restored or manual), so skip silently.
		store.autoZoomApplied = true;
		return;
	}
	await runAutoZoom({ silentEmpty: true });
}

async function runAutoZoom(opts: { silentEmpty?: boolean; undoOnError?: boolean } = {}) {
	if (autoZoomRunning) return;
	if (!cursorPath) return;
	autoZoomRunning = true;
	try {
		// generateAutoZoom latches store.autoZoomApplied itself on non-error paths.
		const outcome = await generateAutoZoom(store, cursorPath, {
			documentPath,
		});
		if (outcome.reason === "bad-bounds") return;
		if (outcome.applied > 0) {
			toast.success(`Added ${outcome.applied} focus moment${outcome.applied === 1 ? "" : "s"}`, {
				description: "Tweak, remove, or turn off in the Focus panel.",
				action: {
					label: "Undo",
					onClick: () => {
						store.clearAutoZooms();
						store.autoZoomApplied = false;
					},
				},
			});
		} else if (!opts.silentEmpty) {
			toast.info("No focus candidates found");
		}
	} catch (err) {
		console.warn("Auto-zoom failed:", err);
		// Analysis throws before mutating, so the only change is the caller's clear, which pushed undo.
		toast.error("Couldn't generate focus moments", {
			description: opts.undoOnError
				? "Your previous focus moments were removed. Undo to bring them back."
				: undefined,
			action: opts.undoOnError ? { label: "Undo", onClick: () => store.undo() } : undefined,
		});
	} finally {
		autoZoomRunning = false;
	}
}

function regenerateAutoZoom() {
	// Only offer Undo when the clear pushed an entry, or the button reverts whatever edit came before.
	const hadAuto = store.zoomRegions.some((z) => z.source === "auto");
	store.clearAutoZooms();
	store.autoZoomApplied = false;
	void runAutoZoom({ silentEmpty: false, undoOnError: hadAuto });
}

// exportActivity owns the queue and run; this editor only tracks the item it enqueued.
let myExportId = $state<string | null>(null);
const myItem = $derived(myExportId ? exportActivity.item(myExportId) : null);
// True while this editor rasterizes its edits before enqueue: the frontend 'preparing' window.
let buildingExport = $state(false);
const isExportingHere = $derived(buildingExport || myItem?.status === "running");

let exportNow = $state<number>(Date.now());
const exportStartedAt = $derived(myItem?.startedAt ?? 0);
const exportCancelling = $derived(myItem?.phase === "cancelling");
const exportFinalizing = $derived(myItem?.phase === "finalizing");
const exportHasProgress = $derived((myItem?.progress ?? 0) > 0);
// Items ahead of this editor's queued export (the running one counts).
const queueAhead = $derived(myExportId ? exportActivity.queuePosition(myExportId) : 0);

// prepare = snapshot/rasterize or pre-first-frame; render = frame loop; finalise = mux or FFmpeg tail.
type ExportStage = "prepare" | "render" | "finalise";
const exportStage = $derived<ExportStage>(
	exportFinalizing ? "finalise" : buildingExport || !exportHasProgress ? "prepare" : "render",
);

// Raw FFmpeg progress is jumpy, so lerp the ring toward it each animation tick.
let displayPct = $state(0);
let easeRafHandle: number | null = null;
$effect(() => {
	if (myItem?.status !== "running") {
		if (easeRafHandle !== null) {
			cancelAnimationFrame(easeRafHandle);
			easeRafHandle = null;
		}
		displayPct = 0;
		return;
	}
	let lastTs: number | null = null;
	function step(now: number) {
		const target = exportFinalizing ? 99.5 : Math.min(99.5, Math.max(0, myItem?.progress ?? 0));
		const dt = lastTs === null ? 16 : Math.max(1, Math.min(64, now - lastTs));
		lastTs = now;
		// Critically-damped follower (~250ms tau): ease-out toward target, no overshoot.
		const tau = 250;
		const k = 1 - Math.exp(-dt / tau);
		const next = displayPct + (target - displayPct) * k;
		// Never animate backwards; the export is monotonic so the ring should be too.
		displayPct = Math.max(displayPct, next);
		easeRafHandle = requestAnimationFrame(step);
	}
	easeRafHandle = requestAnimationFrame(step);
	return () => {
		if (easeRafHandle !== null) {
			cancelAnimationFrame(easeRafHandle);
			easeRafHandle = null;
		}
	};
});

// ETA from elapsed × (1 − pct) / pct; only meaningful past ≥10% progress.
function exportEtaMs(): number | null {
	return computeExportEtaMs({
		hasProgress: exportHasProgress,
		finalizing: exportFinalizing,
		progress: myItem?.progress ?? 0,
		now: exportNow,
		startedAt: exportStartedAt,
	});
}

// Terminal result of THIS editor's export; the store owns the run, toasts and notification.
type ExportResult =
	| { kind: "success"; path: string }
	| { kind: "cancelled" }
	| { kind: "error"; message: string };
const exportResult = $derived<ExportResult | null>(
	myItem?.status === "success"
		? { kind: "success", path: myItem.path ?? "" }
		: myItem?.status === "cancelled"
			? { kind: "cancelled" }
			: myItem?.status === "error" || myItem?.status === "interrupted"
				? { kind: "error", message: myItem.error ?? "" }
				: null,
);

async function handleExport() {
	if (isExportingHere) return;
	const exportId = createExportId();
	buildingExport = true;
	myExportId = exportId;
	exportActivity.show(exportId);
	exportNow = Date.now();

	try {
		// Engine first: it decides whether the render state needs its visual half. The beta toggle is the gate.
		const wantBrowser = experimentalStore.isEnabled("browserExportBeta");
		const capability = wantBrowser ? await probeBrowserExportCapability() : null;
		const engine = chooseExportEngine({
			masterEnabled: wantBrowser,
			blockedReason: browserExportBlockedReason(store),
			capabilitySupported: capability?.supported ?? false,
		});
		log.info("export", "export_engine", {
			exportId,
			engine: engine.engine,
			reason: engine.reason,
			hardwareAccelerated: capability?.hardwareAccelerated ?? false,
		});

		// The browser engine composites text and cursor itself in buildExportJob, so skip that raster here.
		const { renderState: finalRenderState, metadata: meta } = await buildExportRenderState(store, {
			skipVisualRaster: engine.engine === "browser",
		});

		// Warn but don't block: the export otherwise drops unloadable image annotations silently.
		const missingImages = await findMissingImageAnnotations(store);
		if (missingImages.length > 0) {
			const names = missingImages.map(basename).join(", ");
			toast.warning(
				`${missingImages.length} image${missingImages.length > 1 ? "s" : ""} couldn't be found and won't appear in the export: ${names}`,
			);
		}

		// A blur can't follow a zoom in the export, so warn before a redaction slides off its target.
		if (hasBlurUnderZoom(store)) {
			toast.warning(
				"A blur overlaps a zoom. In the export it can't follow the zoom and may not cover the zoomed content. Set the blur's Anchor to Frame if it should stay in a fixed spot.",
			);
		}

		// The settings this export ran with, key when a user reports a bad export.
		log.info("export", "export_started", {
			exportId,
			format: store.exportFormat,
			quality: store.exportQuality,
			speed: store.exportSpeed,
			gif: store.exportFormat === "gif" ? store.gifSettings : undefined,
			annotations: finalRenderState.annotations.length,
			zoomRegions: finalRenderState.zoomRegions.length,
			cuts: finalRenderState.cuts?.length ?? 0,
			padding: finalRenderState.padding ?? 0,
			durationSec: meta ? Math.round(meta.duration) : undefined,
		});

		// `filePath` is the stable route path; `inputPath` is the actual media.
		const params = {
			inputPath: documentPath || data.filePath,
			format: store.exportFormat,
			quality: store.exportQuality,
			renderState: finalRenderState,
			gifSettings: store.exportFormat === "gif" ? store.gifSettings : undefined,
			speed: store.exportSpeed,
			// GIF carries fps in gifSettings; MP4/WebM use the picker (null=source).
			fps: store.exportFormat === "gif" ? undefined : store.exportFps,
			// No-op unless a transcript exists and caption options are enabled.
			captions: buildCaptionExport(store),
			// `keptTimeMap`, not `timeMap`: the latter un-collapses mid trim-drag and would export the trimmed head and tail.
			timeMap: exportTimeMap(store.keptTimeMap),
		};

		// Source metrics the export time is correlated against; the store emits them on `export_completed`.
		const src = store.metadata;
		const telemetry: ExportTelemetry = {
			engine: engine.engine === "browser" ? "browser" : "rust",
			format: store.exportFormat,
			quality: String(store.exportQuality),
			outputDurationSec: store.timeMap.outputDuration,
			srcDurationSec: src?.duration ?? 0,
			srcWidth: src?.width ?? 0,
			srcHeight: src?.height ?? 0,
			srcFps: src?.fps ?? 0,
			srcCodec: src?.codec ?? "",
			srcBytes: src?.sizeBytes ?? 0,
		};

		if (engine.engine === "browser") {
			// Browser render shares this GPU and decoder, so stop playback; the frame stays up and scrubbable.
			store.isPlaying = false;
			// Shared with the eligibility gate, so a source it deemed light renders at the fps it judged.
			const renderFps = resolveExportFps(store);
			// Snapshot while the store is alive, so the export survives closing this editor and queues behind others.
			const job = await buildExportJob(store, {
				videoUrl: videoSrc,
				cameraUrl: cameraSrc,
				cameraOffsetMs: trackOffsets.cameraMs,
				quality: store.exportQuality as ExportQuality,
				fps: renderFps,
			});
			exportActivity.enqueueBrowserExport({
				id: exportId,
				filename: data.filename,
				filePath: data.filePath,
				job,
				params,
				telemetry,
			});
		} else {
			exportActivity.enqueue({
				id: exportId,
				filename: data.filename,
				filePath: data.filePath,
				params,
				telemetry,
			});
		}
	} catch (err) {
		const message =
			typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
		console.error("Export prep failed:", err);
		log.error("export", "export_failed", { exportId, message });
		toast.error(`Couldn't prepare the export: ${message}`);
		myExportId = null;
	} finally {
		buildingExport = false;
	}
}

// The store stops a running export, or drops it from the queue if it hasn't started.
function handleCancelExport() {
	if (myExportId) void exportActivity.cancel(myExportId);
}

function dismissExportResult() {
	if (myExportId) exportActivity.dismiss(myExportId);
	myExportId = null;
	exportActivity.minimize();
}

// Size and created come from the exports listing; a minimal entry keeps playback off that listing's success.
let playTarget = $state<RecordingEntry | null>(null);

async function playExportedFile() {
	if (exportResult?.kind !== "success") return;
	const path = exportResult.path;
	const filename = basename(path) ?? "export";
	let entry: RecordingEntry = {
		filename,
		path,
		sizeBytes: 0,
		created: Math.floor(Date.now() / 1000),
		modified: Math.floor(Date.now() / 1000),
		needsMigration: false,
	};
	try {
		const found = (await listExports()).find((e) => e.path === path);
		if (found) entry = found;
	} catch {
		// Keep the fallback entry.
	}
	playTarget = entry;
	dismissExportResult();
}

// Options is UI-only; progress and result derive from pipeline state, so one surface morphs.
let exportOptionsOpen = $state(false);
// The panel reflects only the export this editor enqueued; a queued item waits behind the running one.
const exportPhase: ExportPanelPhase | null = $derived(
	buildingExport
		? "progress"
		: myItem?.status === "queued"
			? "queued"
			: myItem?.status === "running"
				? "progress"
				: exportResult?.kind === "success"
					? "success"
					: exportResult?.kind === "cancelled"
						? "cancelled"
						: exportResult?.kind === "error"
							? "error"
							: exportOptionsOpen
								? "options"
								: null,
);
// Minimizing keeps the export alive and hands tracking to the activity center.
const isExportFlowOpen = $derived(exportPhase !== null && exportActivity.foreground);

// The panel takes focus on open, so without a return target closing it strands focus on <body>.
let exportReturnFocus: HTMLElement | null = null;
let exportWasOpen = false;
$effect(() => {
	const open = isExportFlowOpen;
	if (!open && exportWasOpen) {
		const el = exportReturnFocus;
		exportReturnFocus = null;
		// After the panel unmounts and the rail re-renders back to the editor.
		if (el?.isConnected) requestAnimationFrame(() => el.focus());
	}
	exportWasOpen = open;
});

// Drives the toolbar Export button: open, close the picker, minimize to the activity center, or reopen.
type ExportButtonMode = "export" | "close" | "minimize" | "show";
const exportButtonMode: ExportButtonMode = $derived(
	exportPhase === null
		? "export"
		: exportPhase === "options"
			? "close"
			: exportActivity.foreground
				? "minimize"
				: "show",
);

function onExportButton() {
	switch (exportButtonMode) {
		case "export":
			// Remember the Export button so focus returns here when the flow closes.
			exportReturnFocus = document.activeElement as HTMLElement | null;
			openExportOptions();
			break;
		case "close":
			dismissExportOptions();
			break;
		case "minimize":
			exportActivity.minimize();
			break;
		case "show":
			exportActivity.show(myExportId);
			break;
	}
}

// Silence cuts only: manual ripple deletes must not trip the 'enable Silence detection' banner.
const silenceCutCount = $derived(store.cuts.filter((c) => c.source === "silence").length);

function openExportOptions() {
	if (isExportingHere) return;
	exportActivity.show();
	exportOptionsOpen = true;
}

function dismissExportOptions() {
	exportOptionsOpen = false;
	exportActivity.minimize();
}

function confirmExportOptions() {
	exportOptionsOpen = false;
	void handleExport();
}

// Esc per phase: cancel a running export, dismiss a finished one, or close the picker.
function handleExportEscape() {
	if (myItem?.status === "running" || myItem?.status === "queued") {
		handleCancelExport();
		return;
	}
	if (exportResult) {
		dismissExportResult();
		return;
	}
	if (exportOptionsOpen) {
		dismissExportOptions();
	}
}

async function copyExportError() {
	if (exportResult?.kind !== "error") return;
	try {
		await navigator.clipboard.writeText(exportResult.message);
		toast.success("Error details copied");
	} catch {
		toast.error("Could not copy to clipboard");
	}
}

async function copyExportPath() {
	if (exportResult?.kind !== "success" || !exportResult.path) return;
	try {
		await navigator.clipboard.writeText(exportResult.path);
		toast.success("Path copied");
	} catch {
		toast.error("Could not copy to clipboard");
	}
}

async function revealExportInFolder() {
	if (exportResult?.kind !== "success") return;
	try {
		await openFileLocation(exportResult.path);
	} catch (err) {
		toast.error(`Could not open folder: ${err}`);
	}
}

// `init()` is a round-trip; without this the button sat dead and got re-clicked, uploading one export three times.
let checkingDestination = $state<"cloud" | "drive" | null>(null);

const exportPath = $derived(exportResult?.kind === "success" ? exportResult.path : null);
const cloudTile = $derived(
	destinationTile(
		{ idle: "Recast Cloud", done: "Copy link" },
		{
			checking: checkingDestination === "cloud",
			phase: exportPath ? cloudShare.uploads[exportPath]?.status : undefined,
			hasRecord: exportPath ? Boolean(cloudShare.getRecordForPath(exportPath)) : false,
		},
	),
);
const driveTile = $derived(
	destinationTile(
		{ idle: "Google Drive", done: "Copy link" },
		{
			checking: checkingDestination === "drive",
			phase: exportPath ? uploadForPath(gdrive.uploads, exportPath)?.status : undefined,
			hasRecord: exportPath ? Boolean(gdrive.getRecordForPath(exportPath)) : false,
		},
	),
);

async function copyToClipboard(text: string, label: string) {
	try {
		await navigator.clipboard.writeText(text);
		toast.success(`${label} copied.`);
	} catch {
		toast.error("Could not copy the link.");
	}
}

// Routes to Settings when not connected: connecting opens a browser tab, which can't happen from this card.
async function uploadExportToDrive() {
	if (exportResult?.kind !== "success" || driveTile.disabled) return;
	const path = exportResult.path;
	const link = gdrive.getRecordForPath(path)?.webViewLink;
	// A finished upload turns the tile into its own link, so a second click copies rather than re-uploads.
	if (link && driveTile.status === "done") return copyToClipboard(link, "Drive link");

	checkingDestination = "drive";
	try {
		await gdrive.init();
		if (!gdrive.connected) {
			toast.info("Connect Google Drive in Settings first.");
			void goto(settingsHref("cloud"));
			return;
		}
		// Byte progress lives in the dialog and activity center; the tile only reports state.
		const id = gdrive.startUpload(path);
		requestAnimationFrame(() => gdrive.setForeground(id));
	} finally {
		checkingDestination = null;
	}
}

// Shares the export to Recast Cloud and copies the link; routes to Settings if not signed in.
async function shareCurrentExportToCloud() {
	if (exportResult?.kind !== "success" || cloudTile.disabled) return;
	const path = exportResult.path;
	const shareUrl = cloudShare.getRecordForPath(path)?.shareUrl;
	if (shareUrl && cloudTile.status === "done") return copyToClipboard(shareUrl, "Share link");

	checkingDestination = "cloud";
	try {
		await cloudShare.init();
		if (!cloudShare.signedIn) {
			toast.info("Sign in to Recast Cloud in Settings first.");
			void goto(settingsHref("cloud"));
			return;
		}
	} finally {
		checkingDestination = null;
	}

	const title = basename(path)?.replace(/\.[^.]+$/, "") ?? "Recast";
	// The rAF before foregrounding lets a closing overlay settle, or bits-ui takes focus back and the dialog never appears.
	const shared = cloudShare
		.share(path, title, undefined, buildCloudCaptionTranscript(store))
		.catch(() => null);
	requestAnimationFrame(() => cloudShare.setForeground(path));
	const result = await shared;
	if (!result) return;
	try {
		await navigator.clipboard.writeText(result.shareUrl);
	} catch {
		// Clipboard blocked; the link is still in the dialog and activity center.
	}
}

// Exposure is static, so sample once; the OS name marks the sheet ('Windows share' beats a generic node).
const shareSupported = isShareSupported();
const shareTarget = shareTargetFor(platform());

async function shareExportedFile() {
	if (exportResult?.kind !== "success") return;
	const fileName = basename(exportResult.path) ?? "recording";
	// OS share sheets can't attach a local file everywhere; fall back to a recorded Drive link.
	const fallbackLink = gdrive.getRecordForPath(exportResult.path)?.webViewLink;
	const result = await shareRecording({
		path: exportResult.path,
		fileName,
		title: fileName,
		text: "Made with Recast",
		fallbackLink,
	});
	if (result.ok || result.reason === "cancelled") return;
	if (result.reason === "unsupported") {
		toast.error(
			fallbackLink
				? "Sharing isn't available on this device."
				: "Sharing files isn't available here. Upload to Drive first to share a link.",
		);
	} else {
		toast.error(`Share failed: ${result.message ?? "unknown error"}`);
	}
}

function getExportDuration() {
	const duration = store.metadata?.duration ?? 0;
	const clipEnd = store.trimEnd > 0 ? store.trimEnd : duration;
	return Math.max(0, clipEnd - store.trimStart);
}

function getExportRangeLabel() {
	const duration = store.metadata?.duration ?? 0;
	const clipEnd = store.trimEnd > 0 ? store.trimEnd : duration;
	return `${formatClock(store.trimStart)} - ${formatClock(clipEnd)}`;
}

let isSaving = $state(false);

async function handleSave() {
	if (!documentPath || isSaving || isLoading) return;
	isSaving = true;
	// Serialize stays on the main thread: Tauri JSON-encodes command args there anyway, so a worker only adds a clone.
	await tick();
	try {
		const editsJson = JSON.stringify(store.toRenderState());
		const savedAt = await saveProjectEdits(documentPath, editsJson);
		store.markSaved(savedAt);
		toast.success("Saved");
	} catch (err) {
		const message =
			typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
		toast.error(`Couldn't save: ${message}`);
	} finally {
		isSaving = false;
	}
}

// Each handler bails while the export flow dialog owns the screen.
onMount(() =>
	registerShortcutHandlers({
		"editor.undo": () => {
			if (!isExportFlowOpen) store.undo();
		},
		"editor.redo": () => {
			if (!isExportFlowOpen) store.redo();
		},
		"editor.save": () => {
			if (!isExportFlowOpen) void handleSave();
		},
		"editor.toggleSidebar": () => {
			if (!isExportFlowOpen) showSidebar = !showSidebar;
		},
		"editor.toggleTimeline": () => {
			if (!isExportFlowOpen) showTimeline = !showTimeline;
		},
	}),
);

function handleKeydown(e: KeyboardEvent) {
	// Bail on auto-repeat so a held key counts once.
	if (e.defaultPrevented || e.repeat) return;

	// The export panel owns Esc routing while open; bail so global shortcuts don't fire behind it.
	if (isExportFlowOpen) return;

	// Never hijack typing in inputs / textareas / contenteditable.
	const target = e.target;
	if (
		target instanceof HTMLInputElement ||
		target instanceof HTMLTextAreaElement ||
		(target instanceof HTMLElement && target.isContentEditable)
	) {
		return;
	}

	// Mod combos belong to the central registry; bail so one never trips a plain-key action below.
	if (e.ctrlKey || e.metaKey) return;

	// Document scope, not scroller focus, or the toolbar keycaps lie; Shift/Alt variants stay scroller-local.
	const runTimelineCommand = (run: (c: NonNullable<typeof store.timelineCommands>) => void) => {
		const c = store.timelineCommands;
		if (!c || !store.metadata || e.shiftKey || e.altKey || isOverlayOpen()) return;
		e.preventDefault();
		run(c);
	};

	// Plain keys: play/pause, frame step, fullscreen.
	switch (e.key) {
		case " ":
			// Buttons fire click on Space KEYUP, so preventing keydown here would leave every focused control dead.
			if (activatesOnSpace(document.activeElement)) return;
			e.preventDefault();
			if (!videoEl) return;
			if (store.isPlaying) {
				videoEl.pause();
				store.isPlaying = false;
			} else {
				videoEl.play();
				store.isPlaying = true;
			}
			break;
		case "ArrowLeft":
			if (store.metadata) frameStepSeek(-1);
			break;
		case "ArrowRight":
			if (store.metadata) frameStepSeek(1);
			break;
		case "f":
		case "F":
			e.preventDefault();
			if (document.fullscreenElement) {
				void document.exitFullscreen();
			} else if (previewContainerEl) {
				void previewContainerEl.requestFullscreen();
			}
			break;
		// Delete acts on the SELECTION, not DOM focus; three surfaces used to claim it and could remove two objects at once.
		case "Delete":
		case "Backspace": {
			const removed = store.deleteSelection();
			if (!removed) return;
			e.preventDefault();
			// A clip delete closes the gap; park the playhead on the join so it lands on a kept frame.
			if (removed.joinAt !== null) {
				store.seek(removed.joinAt);
				if (videoEl) videoEl.currentTime = removed.joinAt;
			}
			break;
		}
		case "s":
		case "S":
			runTimelineCommand((c) => c.splitAtPlayhead());
			break;
		case "c":
		case "C":
			runTimelineCommand((c) => c.toggleRazor());
			break;
		case "i":
		case "I":
			runTimelineCommand((c) => c.trimToPlayhead("in"));
			break;
		case "o":
		case "O":
			runTimelineCommand((c) => c.trimToPlayhead("out"));
			break;
		case "Home":
			runTimelineCommand((c) => c.seekToEdge("in"));
			break;
		case "End":
			runTimelineCommand((c) => c.seekToEdge("out"));
			break;
		case "Escape":
			// Exit an armed tool before deselecting; the annotation overlay preventDefaults its own Escape, so we never fight it.
			if (store.timelineTool === "razor") {
				store.timelineCommands?.exitTool();
				e.preventDefault();
			} else if (store.selection) {
				store.clearSelection();
				e.preventDefault();
			}
			break;
	}
}

$effect(() => {
	if (!data.filePath || data.filePath === loadedPath) return;
	loadedPath = data.filePath;
	void loadDocument();
});

$effect(() => {
	if (!videoEl) return;
	videoEl.muted = true;
});

// Idle-deferred so the ffmpeg pass never runs on the load path; the latch stops re-runs re-scheduling it.
$effect(() => {
	if (store.waveform.length > 0 || waveformRequested) return;
	if (!store.audioPath && !store.microphonePath) return;
	waveformRequested = true;
	const run = () => void loadWaveform();
	if (typeof requestIdleCallback === "function") {
		requestIdleCallback(run, { timeout: 3000 });
	} else {
		setTimeout(run, 1000);
	}
});

$effect(() => {
	if (myItem?.status !== "running") return;
	exportNow = Date.now();
	// Elapsed-time timer for the status strip.
	const timer = setInterval(() => {
		exportNow = Date.now();
	}, 500);
	return () => clearInterval(timer);
});

// The 3-stage rail order, for the minimal progress dots under the ring.
const EXPORT_STAGES: ExportStage[] = ["prepare", "render", "finalise"];
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="fixed inset-0 flex min-h-screen w-full flex-col overflow-hidden bg-background text-foreground"
>
  <CustomTitlebar wrapperClass="h-9">
    <!-- `inert` (not per-control `disabled`) so a future toolbar action can't
         silently miss the gate: it blocks pointer + tab + a11y tree in one. -->
    <div class="flex min-w-0 flex-1 items-center" inert={agentSession.active}>
      <EditorToolbar
        {store}
        filename={data.filename}
        onexport={onExportButton}
        exportMode={exportButtonMode}
        exportRunning={myItem?.status === "running"}
        onsave={handleSave}
        {isSaving}
        {showSidebar}
        {showTimeline}
        onToggleSidebar={() => (showSidebar = !showSidebar)}
        onToggleTimeline={() => (showTimeline = !showTimeline)}
      />
    </div>
    <AgentSessionBadge />
    {#if documentPath}
      <BranchReviewPanel
        projectPath={documentPath}
        writerId={editorWriterId}
        onPreview={(state) => {
          store.pushUndoState();
          store.loadRenderState(state);
        }}
        onApplied={() => void reloadRenderStateFromDisk()}
      />
    {/if}
  </CustomTitlebar>

  <!-- Foreground upload dialogs (cloud share + Drive), reopened by clicking an
       upload in the activity center; store-driven so they survive navigation. -->
  <UploadDialogsHost />

  <ConfirmDialog
    bind:open={showMigration}
    title="Update project format"
    description="This project was made with an older version of Recast. Update it to the current format to keep editing. A backup (.bak) is saved next to it first."
    confirmLabel="Update project"
    cancelLabel="Not now"
    onConfirm={confirmMigration}
    onOpenChange={onMigrationOpenChange}
  />

  <!-- Project has silence cuts but the flag is off, so they're hidden and
       skipped on export, so surface an inline opt-in so work isn't lost. -->
  {#if !isLoading && !error && silenceCutCount > 0 && !experimentalStore.silenceDetection}
    <div
      class="flex items-center gap-2.5 border-b border-warning/30 bg-warning/10 px-3 py-1.5 text-[11px] text-warning"
      role="status"
    >
      <FlaskConical class="size-3.5 shrink-0" />
      <VolumeX class="size-3.5 shrink-0" />
      <span class="min-w-0 flex-1 truncate">
        This project has {silenceCutCount} silence cut{silenceCutCount === 1
          ? ""
          : "s"}, currently hidden and skipped on export. Enable
        <span class="font-semibold">Silence detection</span> to use them.
      </span>
      <Button
        variant="outline"
        size="xs"
        class="h-6 shrink-0 border-warning/40 bg-warning/10 text-warning hover:bg-warning/20"
        onclick={() =>
          experimentalStore.setEnabled("silenceDetection", true)}
      >
        Enable
      </Button>
    </div>
  {/if}

  {#if isLoading}
    <EditorSkeleton />
  {:else if error}
    <div class="flex flex-1 items-center justify-center">
      <div
        class="animate-in fade-in flex max-w-sm flex-col items-center gap-3 text-center duration-500"
      >
        <div
          class="flex size-10 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive"
        >
          <span class="text-[18px] font-semibold">!</span>
        </div>
        <p class="text-[12px] text-muted-foreground">{error}</p>
        <Button
          variant="outline"
          size="sm"
          href="/recasts"
          class="gap-1.5"
        >
          <ArrowLeft size={13} />
          Back to recordings
        </Button>
      </div>
    </div>
  {:else}
    <!-- h-auto drops Editor's own h-full via tailwind-merge: this is a flex
         child sharing the column with the titlebar, so flex-1 owns its height. -->
    <Editor
      {store}
      services={tauriEditorServices}
      {videoSrc}
      {cursorPath}
      {cameraSrc}
      cameraOffsetMs={trackOffsets.cameraMs}
      {cameraPath}
      {cameraCapture}
      {audioEngine}
      {tileProvider}
      {filmstripVersion}
      bind:showSidebar
      bind:showTimeline
      bind:videoEl
      bind:previewContainerEl
      bind:captureFrame
      bind:webcodecsActive
      bind:loopEnabled
      onTimeUpdate={handleTimeUpdate}
      onEnded={handleVideoEnded}
      onLoadedMetadata={handleVideoLoadedMetadata}
      onReady={handleVideoReady}
      onError={handleVideoError}
      onSeeked={handleVideoSeeked}
      audioPositionSec={() => audioEngine?.positionOutputSec ?? null}
      onRegenerateAutoZoom={regenerateAutoZoom}
      timelineReadOnly={agentSession.active}
      panelReadOnly={agentSession.active}
      toolbar={hostOwnsToolbar}
      exportPanel={isExportFlowOpen ? exportRail : undefined}
      class="h-auto min-h-0 flex-1"
    />
  {/if}

  {#if playTarget}
    <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
  {/if}
</div>

<!-- Renders nothing on purpose: this window's toolbar lives in the native
     CustomTitlebar above the shell, so Editor must not draw its default one. -->
{#snippet hostOwnsToolbar()}{/snippet}

{#snippet exportRail()}
  <ExportPanel
    phase={exportPhase}
    onEscape={handleExportEscape}
    {options}
    {queued}
    {progress}
    {success}
    {cancelled}
    error={errorPanel}
  />
{/snippet}

{#snippet options()}
  <ExportDialog
    {store}
    onConfirm={confirmExportOptions}
    onCancel={dismissExportOptions}
  />
{/snippet}

{#snippet queued()}
  <div class="flex h-full min-h-0 flex-col">
    <header class="shrink-0 border-b border-border/40 px-5 pb-3.5 pt-4">
      <div class="min-w-0">
        <h3
          id="export-flow-title"
          class="text-[15px] font-semibold tracking-tight text-foreground"
        >
          Queued for export
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Waiting for the current export to finish. You can keep working.
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <div class="min-h-0 flex-1 overflow-y-auto scrollbar-transparent">
      <div
        class="mx-auto flex min-h-full w-full max-w-xs flex-col items-center justify-center gap-3 px-5 py-6 text-center"
      >
        <span class="relative flex size-3 items-center justify-center">
          <span
            class="absolute inline-flex size-full rounded-full bg-primary/40 motion-safe:animate-ping"
          ></span>
          <span
            class="relative inline-flex size-2.5 rounded-full bg-primary"
          ></span>
        </span>
        <p class="text-[11px] text-muted-foreground">
          {queueAhead > 0
            ? `${queueAhead} export${queueAhead === 1 ? "" : "s"} ahead of yours`
            : "Starting soon…"}
        </p>
      </div>
    </div>

    <footer
      class="flex shrink-0 items-center justify-end gap-1.5 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button
        variant="destructive_soft"
        size="xs"
        class="gap-1.5"
        onclick={handleCancelExport}
      >
        <X class="size-3" />
        Remove from queue
      </Button>
    </footer>
  </div>
{/snippet}

{#snippet exportSpecStrip()}
  {@const fmt = store.exportFormat}
  {@const isGifFmt = fmt === "gif"}
  {@const srcFps = Math.max(1, Math.round(store.metadata?.fps ?? 60))}
  {@const qualityLabel = isGifFmt
    ? store.gifSettings.quality === "low"
      ? "Lite"
      : store.gifSettings.quality === "high"
        ? "Vivid"
        : "Standard"
    : store.exportQuality === "small"
      ? "720p"
      : store.exportQuality === "hd"
        ? "1080p"
        : store.exportQuality === "4k"
          ? "2160p"
          : "Source"}
  {@const fpsLabel = isGifFmt
    ? store.gifSettings.fps
      ? `${store.gifSettings.fps}`
      : "Auto"
    : store.exportFps
      ? `${store.exportFps}`
      : `${srcFps}`}
  <!-- Carries the committed export settings forward so every later phase stays
       anchored to "what you're exporting". Same shape as the options header so
       the panel doesn't restructure itself the moment you press Export. -->
  <section class="border-b border-border/40 px-5 py-2.5">
    <dl class="grid grid-cols-4 gap-x-3">
      <div class="flex min-w-0 flex-col gap-0.5">
        <dt class="text-[11px] text-muted-foreground">Format</dt>
        <dd class="truncate text-[12px] font-medium text-foreground">{fmt.toUpperCase()}</dd>
      </div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <dt class="text-[11px] text-muted-foreground">{isGifFmt ? "Colors" : "Quality"}</dt>
        <dd class="truncate text-[12px] font-medium text-foreground">{qualityLabel}</dd>
      </div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <dt class="text-[11px] text-muted-foreground">FPS</dt>
        <dd class="truncate font-mono text-[12px] tabular-nums text-foreground">{fpsLabel}</dd>
      </div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <dt class="text-[11px] text-muted-foreground">Duration</dt>
        <dd class="truncate font-mono text-[12px] tabular-nums text-foreground">
          {formatClock(getExportDuration())}
        </dd>
      </div>
    </dl>
  </section>
{/snippet}

{#snippet progress()}
  {@const isPreparing = !exportHasProgress && !exportFinalizing}
  {@const eta = exportEtaMs()}

  <div class="flex h-full min-h-0 flex-col">
    <header class="shrink-0 border-b border-border/40 px-5 pb-3.5 pt-4">
      <div class="min-w-0">
        <h3
          id="export-flow-title"
          class="text-[15px] font-semibold tracking-tight text-foreground"
        >
          {#if exportCancelling}
            Cancelling export…
          {:else if exportFinalizing}
            Finalising file
          {:else if isPreparing}
            Preparing export
          {:else}
            Rendering video
          {/if}
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          {#if exportCancelling}
            Stopping and cleaning up the partial file…
          {:else if exportFinalizing}
            Writing the finished file to disk…
          {:else if isPreparing}
            Getting frames and effects ready…
          {:else}
            This can take a moment. You can keep working.
          {/if}
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <div class="min-h-0 flex-1 overflow-y-auto scrollbar-transparent">
      <div
        class="mx-auto flex min-h-full w-full max-w-xs flex-col items-center justify-center gap-5 px-5 py-6"
      >
      <ExportStageLoader stage={exportStage} pct={displayPct} />

      {#if !exportCancelling}
        <p class="text-[11px] tabular-nums text-muted-foreground">
          {#if eta !== null}
            ~{formatElapsed(eta)} left
          {:else if exportStartedAt}
            {formatElapsed(exportNow - exportStartedAt)} elapsed
          {/if}
        </p>
      {/if}

      <!-- Minimal 3-stage rail: past stages filled, the current one widened. -->
      <div class="flex items-center gap-2" aria-hidden="true">
        {#each EXPORT_STAGES as st, i (st)}
          {@const active = st === exportStage}
          {@const done = EXPORT_STAGES.indexOf(exportStage) > i}
          <span
            class="h-1.5 rounded-full transition-all duration-300 {active
              ? 'w-6 bg-primary'
              : done
                ? 'w-1.5 bg-primary/60'
                : 'w-1.5 bg-muted-foreground/25'}"
          ></span>
        {/each}
      </div>
          </div>
    </div>

    <footer
      class="flex shrink-0 items-center justify-end gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button
        variant="destructive_soft"
        size="xs"
        class="gap-1.5"
        onclick={handleCancelExport}
        disabled={exportCancelling}
      >
        <X class="size-3" />
        {exportCancelling ? "Cancelling…" : "Cancel export"}
      </Button>
    </footer>
  </div>
{/snippet}

<!-- `hint?:` survives into the emitted snippet function as `hint?`, which is not
     valid JavaScript: template types are not preprocessed the way script ones are. -->
{#snippet destination(
  Icon: IconComponent,
  tile: DestinationTile,
  onclick: () => void,
  hint: string | undefined = undefined,
)}
  <button
    type="button"
    {onclick}
    disabled={tile.disabled}
    title={hint}
    aria-busy={tile.status === "busy"}
    class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border px-3 py-3 text-center transition-colors duration-150 disabled:cursor-default {tile.status ===
    'error'
      ? 'border-destructive/40 bg-destructive/5 hover:border-destructive/60'
      : 'border-border/50 bg-card/60 not-disabled:hover:border-border not-disabled:hover:bg-card'}"
  >
    <span
      class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 shadow-(--shadow-craft-inset) transition-colors {tile.status ===
      'error'
        ? 'text-destructive'
        : tile.status === 'done'
          ? 'text-success'
          : 'text-muted-foreground group-hover/dest:text-primary'}"
    >
      {#if tile.status === "busy"}
        <Spinner class="size-4" />
      {:else if tile.status === "done"}
        <CheckCircle2 class="size-4" />
      {:else if tile.status === "error"}
        <TriangleAlert class="size-4" />
      {:else}
        <Icon class="size-4" />
      {/if}
    </span>
    <span class="text-[11px] font-medium leading-none text-foreground">
      {tile.label}
    </span>
  </button>
{/snippet}

{#snippet success()}
  <div class="flex h-full min-h-0 flex-col">
    <header class="shrink-0 border-b border-border/40 px-5 pb-3.5 pt-4">
      <h3
        id="export-flow-title"
        class="flex items-center gap-2 text-[15px] font-semibold tracking-tight text-foreground"
      >
        <CheckCircle2 class="size-4 text-success" />
        Export complete
      </h3>
      {#if myItem?.startedAt != null && myItem?.finishedAt != null}
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Exported in {formatElapsed(myItem.finishedAt - myItem.startedAt)}
        </p>
      {/if}
      {#if exportResult?.kind === "success"}
        <!-- Where it went is the point of this screen, so the path is
             selectable and copyable rather than a truncated tooltip. -->
        <div class="mt-2 flex items-center gap-1.5">
          <p
            class="min-w-0 flex-1 select-text truncate font-mono text-[11px] text-muted-foreground"
            title={exportResult.path}
          >
            {exportResult.path}
          </p>
          <Button
            variant="ghost"
            size="xs"
            class="shrink-0 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={copyExportPath}
          >
            <Copy class="size-3" />
            Copy
          </Button>
        </div>
      {/if}
    </header>

    {@render exportSpecStrip()}

    <div class="min-h-0 flex-1 overflow-y-auto scrollbar-transparent">
    <!-- Share/upload tiles, grouped out of the footer so they read as one
         "where does this go?" choice. The tiles carry STATE (working, done,
         failed) but never a percentage or bar: byte progress belongs to the
         foreground dialog and the activity center. -->
    <div class="border-t border-border/40 bg-muted/15 px-5 py-3.5">
      <p class="mb-2.5 text-[11px] font-semibold text-foreground">Share or upload</p>
      <div class="flex items-stretch gap-2">
        {@render destination(RecastMark, cloudTile, shareCurrentExportToCloud)}
        {@render destination(BrandGoogleDrive, driveTile, uploadExportToDrive)}
        {#if shareSupported}
          <!-- The OS sheet is its own feedback and finishes with the click, so
               this one has no state to carry. -->
          {@render destination(
            shareTarget.icon,
            { status: "idle", label: shareTarget.label, disabled: false },
            shareExportedFile,
            "Open the system share sheet",
          )}
        {/if}
      </div>
    </div>
    </div>

    <footer
      class="flex shrink-0 items-center justify-between gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button
        variant="ghost"
        size="xs"
        class="gap-1.5 text-muted-foreground"
        onclick={dismissExportResult}
      >
        Dismiss
      </Button>

      <div class="flex items-center gap-1.5">
        <Button
          variant="secondary"
          size="xs"
          class="gap-1.5"
          onclick={playExportedFile}
        >
          <Play class="size-3" />
          Play
        </Button>
        <Button
          variant="default"
          size="xs"
          class="gap-1.5"
          onclick={revealExportInFolder}
        >
          <FolderOpen class="size-3" />
          Show in folder
        </Button>
      </div>
    </footer>
  </div>
{/snippet}

{#snippet cancelled()}
  <div class="flex h-full min-h-0 flex-col">
    <header
      class="shrink-0 border-b border-border/40 px-5 pb-3.5 pt-4"
    >
      <div class="min-w-0">
        <h3
          id="export-flow-title"
          class="text-[15px] font-semibold tracking-tight text-foreground"
        >
          Export cancelled
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Stopped before finishing, so no file was written. Your settings are
          kept, so you can pick up right where you left off.
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}
    <!-- Nothing was written, so the body is intentionally empty; the spacer
         keeps the strip anchored under the header and the actions at the base. -->
    <div class="min-h-0 flex-1"></div>

    <footer
      class="flex shrink-0 items-center justify-end gap-1.5 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button variant="ghost" size="xs" onclick={dismissExportResult}
        >Dismiss</Button
      >
      <Button
        variant="default"
        size="xs"
        class="gap-1.5"
        onclick={handleExport}
      >
        <Upload class="size-3" />
        Export again
      </Button>
    </footer>
  </div>
{/snippet}

{#snippet errorPanel()}
  <div class="flex h-full min-h-0 flex-col">
    <header
      class="shrink-0 border-b border-border/40 px-5 pb-3.5 pt-4"
    >
      <div class="min-w-0">
        <h3
          id="export-flow-title"
          class="flex items-center gap-2 text-[15px] font-semibold tracking-tight text-foreground"
        >
          <TriangleAlert class="size-4 text-destructive" />
          Export failed
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Something went wrong while encoding. Your settings are kept, so try
          again, or adjust them first.
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <!-- Raw FFmpeg/pipeline message; fills the rail and scrolls so a long stack
         stays contained above the pinned footer. -->
    <div
      class="min-h-0 flex-1 overflow-y-auto border-b border-border/40 px-5 py-3 scrollbar-transparent"
    >
      <div class="mb-1.5 flex items-center justify-between gap-2">
        <p class="text-[11px] font-semibold text-foreground">Details</p>
        {#if exportResult?.kind === "error"}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={copyExportError}
          >
            <Copy class="size-3" />
            Copy
          </Button>
        {/if}
      </div>
      {#if exportResult?.kind === "error"}
        <pre
          class="whitespace-pre-wrap wrap-break-word font-mono text-[10px] leading-snug text-destructive">{exportResult.message}</pre>
      {/if}
    </div>
    <footer
      class="flex shrink-0 items-center justify-end gap-1.5 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button variant="ghost" size="xs" onclick={dismissExportResult}
        >Dismiss</Button
      >
      <Button
        variant="default"
        size="xs"
        class="gap-1.5"
        onclick={handleExport}
      >
        <Upload class="size-3" />
        Try again
      </Button>
    </footer>
  </div>
{/snippet}

