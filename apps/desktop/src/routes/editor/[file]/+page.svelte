<script lang="ts">
import { browser } from "$app/environment";
import { goto } from "$app/navigation";
import UploadDialogsHost from "$components/cloud/UploadDialogsHost.svelte";
import EditorToolbar from "$components/editor/EditorToolbar.svelte";
import ExportDialog from "$components/editor/ExportDialog.svelte";
import ExportPanel, { type ExportPanelPhase } from "$components/editor/ExportPanel.svelte";
import PropertiesPanel from "$components/editor/properity-panel/PropertiesPanel.svelte";
import Timeline from "$components/editor/Timeline.svelte";
import VideoPlayerControls from "$components/editor/VideoPlayerControls.svelte";
import VideoPreview from "$components/editor/VideoPreview.svelte";
import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
import ConfirmDialog from "$components/recast/ConfirmDialog.svelte";
import PlayerDialog from "$components/recast/PlayerDialog.svelte";
import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
import { activatesOnSpace, isOverlayOpen } from "$lib/dom/keyboard";
import type { RecordingEntry } from "$lib/ipc";
import {
	autosaveProject,
	clearAutosave,
	createExportId,
	detectSilence,
	extractWaveform,
	generateThumbnails,
	listExports,
	loadEditorDocument,
	migrateProject,
	openFileLocation,
	saveProjectEdits,
} from "$lib/ipc";
import { AudioTimelineEngine, type MusicClipSpec } from "$lib/playback/audio-engine";
import { clipAssetPath } from "$lib/audio/music";
import { reconcileAvDrift } from "$lib/playback/av-drift";
import { generateAutoZoom } from "$lib/services/analysis";
import {
	buildCaptionExport,
	buildCloudCaptionTranscript,
	buildExportRenderState,
	findMissingImageAnnotations,
	hasBlurUnderZoom,
} from "$lib/services/export";
import { runBrowserExport } from "$lib/export/browser-export";
import { browserExportBlockedReason } from "$lib/export/browser-export-eligibility";
import type { ExportQuality } from "$lib/export/browser-export-plan";
import { isShareSupported, shareRecording } from "$lib/share";
import { registerShortcutHandlers } from "$lib/shortcuts/registry.svelte";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import { createEditorStore, type VideoMetadata } from "$lib/stores/editor-store.svelte";
import { experimentalStore } from "$lib/stores/experimental.svelte";
import { exportActivity } from "$lib/stores/exportActivity.svelte";
import { gdrive } from "$lib/stores/gdrive.svelte";
import { createTileProvider, type TileProvider } from "$lib/timeline/filmstrip-source";
import { originalToOutput } from "$lib/timeline/time-map";
import { decoderBudget } from "$lib/playback/decoder-budget";
import {
	ArrowLeft,
	CheckCircle2,
	Clock,
	Cloud,
	FlaskConical,
	FolderOpen,
	HardDriveUpload,
	Play,
	Share2,
	TriangleAlert,
	Upload,
	VolumeX,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Kbd } from "@recast/ui/kbd";
import { toast } from "@recast/ui/sonner";
import { convertFileSrc } from "@tauri-apps/api/core";
import { onDestroy, onMount, tick, untrack } from "svelte";

import { formatClock, frameStepOutput } from "$lib/editor/time";
import { log } from "$lib/logger";
import { cubicOut } from "svelte/easing";
import { fade, slide } from "svelte/transition";
import {
	basename,
	exportEtaMs as computeExportEtaMs,
	ENCODE_MESSAGES,
	formatElapsed,
	parseLayout,
} from "./editor-page.logic";

interface Props {
	data: {
		filePath: string;
		filename: string;
	};
}

let { data }: Props = $props();

const store = createEditorStore();

let videoEl: HTMLVideoElement | null = $state(null);
// True while the WebCodecs engine drives the picture (its clock owns
// `store.currentTime`). When set, handleTimeUpdate must NOT echo
// `videoEl.currentTime`, because the element free-runs through the un-cut recording,
// so feeding its time to the store snaps playback back across a cut.
let webcodecsActive = $state(false);
// WYSIWYG screenshot (composite, not raw frame); bound from VideoPreview.
let captureFrame = $state<(() => Promise<Blob | null>) | undefined>(undefined);
// Loop-within-trim. Lives here because both `ended` and `timeupdate` end-of-clip
// paths need handling here, with one source of truth for pause-vs-loop.
let loopEnabled = $state(false);

// Persisted sidebar/timeline visibility; missing or malformed falls back to all visible.
const LAYOUT_KEY = "recast-editor-layout";
function loadLayout(): { sidebar: boolean; timeline: boolean } {
	if (!browser) return { sidebar: true, timeline: true };
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
		// localStorage can throw in private-mode/quota edge cases. The toggle
		// still works for the session, it just won't be remembered.
	}
});

// Resizable properties panel. Width is user-set (drag the splitter or arrow
// keys) and persisted, so a chosen width survives reopening the editor. The
// floor is the panel's old fixed width (w-88, 352px): the dense panels were
// already tight there, so we never let it shrink below it, only grow.
const SIDEBAR_WIDTH_KEY = "recast-editor-sidebar-width";
const SIDEBAR_MIN = 352;
const SIDEBAR_MAX = 600;
const SIDEBAR_DEFAULT = 384;
const clampSidebar = (w: number) => Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(w)));
function loadSidebarWidth(): number {
	if (!browser) return SIDEBAR_DEFAULT;
	const raw = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY));
	return Number.isFinite(raw) && raw > 0 ? clampSidebar(raw) : SIDEBAR_DEFAULT;
}
let sidebarWidth = $state(loadSidebarWidth());
let resizingSidebar = $state(false);
$effect(() => {
	if (!browser) return;
	try {
		localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
	} catch {
		// Best-effort, same as the layout prefs above.
	}
});

// The panel is docked right, so dragging the splitter left widens it: width
// grows as the pointer's x decreases.
function startSidebarResize(e: PointerEvent) {
	if (e.button !== 0) return;
	e.preventDefault();
	resizingSidebar = true;
	const startX = e.clientX;
	const startW = sidebarWidth;
	document.body.style.cursor = "col-resize";
	(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	const onMove = (ev: PointerEvent) => {
		sidebarWidth = clampSidebar(startW - (ev.clientX - startX));
	};
	const onUp = () => {
		resizingSidebar = false;
		document.body.style.cursor = "";
		window.removeEventListener("pointermove", onMove);
		window.removeEventListener("pointerup", onUp);
		window.removeEventListener("pointercancel", onUp);
	};
	window.addEventListener("pointermove", onMove);
	window.addEventListener("pointerup", onUp);
	window.addEventListener("pointercancel", onUp);
}

// Keyboard resize (window-splitter pattern): Left widens, Right narrows, since
// Left moves the splitter toward the panel's growing edge. Home/End jump to the
// bounds. Shift takes a coarser step.
function onSidebarHandleKey(e: KeyboardEvent) {
	const step = e.shiftKey ? 48 : 16;
	switch (e.key) {
		case "ArrowLeft":
			e.preventDefault();
			sidebarWidth = clampSidebar(sidebarWidth + step);
			break;
		case "ArrowRight":
			e.preventDefault();
			sidebarWidth = clampSidebar(sidebarWidth - step);
			break;
		case "Home":
			e.preventDefault();
			sidebarWidth = SIDEBAR_MAX;
			break;
		case "End":
			e.preventDefault();
			sidebarWidth = SIDEBAR_MIN;
			break;
	}
}

let previewContainerEl: HTMLDivElement | null = $state(null);
let systemAudioEl: HTMLAudioElement | null = $state(null);
let micAudioEl: HTMLAudioElement | null = $state(null);
let videoSrc = $state("");
let systemAudioSrc = $state("");
let micAudioSrc = $state("");
// Sample-accurate cut-aware audio for the WebCodecs preview (no seeking → no
// drift). Falls back to the <audio> elements if it can't init/decode.
let audioEngine: AudioTimelineEngine | null = $state(null);
let audioEngineTried = false;
// Bumped whenever the document changes or the editor is destroyed, so an
// engine that finishes decoding afterwards knows it is stale.
let audioEngineGen = 0;
let audioEngineFailed = $state(false);
let cursorPath = $state<string | null>(null);
let cameraPath = $state<string | null>(null);
let cameraSrc = $state("");
let documentPath = $state("");
let isLoading = $state(true);
let error = $state("");
let loadedPath = $state("");
let thumbnailToken = 0;

// Density-based filmstrip: a WebCodecs tile provider (or null, when the clip
// bar falls back to the stretched Rust strip). `filmstripVersion` bumps as
// decoded tiles land so the bar repaints. Clip-bar height (h-12) in CSS px.
const FILMSTRIP_TILE_HEIGHT = 48;
let tileProvider = $state<TileProvider | null>(null);
let filmstripVersion = $state(0);
let tileProviderToken = 0;

// Preview owns decode priority: the shared DecoderBudget pauses the filmstrip's
// decoder while the preview is busy — playing OR scrubbing (seeks thrash the
// decoder while isPlaying is false) — so the two never over-subscribe the GPU's
// decode sessions. The filmstrip registers as a lease in setupTileProvider.
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

// Legacy-format gate: a v1 `.recast` must be migrated before the editor
// touches it. `migrationDone` distinguishes a confirmed update (→ reload)
// from a dismissal (→ leave, don't open an un-migrated project).
let showMigration = $state(false);
let migrationDone = false;

// Autosave: save edit state every 30 seconds while editing.
const AUTOSAVE_INTERVAL_MS = 30_000;
let autosaveTimer: ReturnType<typeof setInterval> | null = null;

function startAutosave() {
	stopAutosave();
	autosaveTimer = setInterval(async () => {
		if (!documentPath || isLoading) return;
		// Skip the full-state serialize when nothing changed since the last
		// save/autosave. Most idle ticks are clean, so the 30s timer stays off
		// the main thread entirely until there's real work to persist.
		if (!store.isDirty) return;
		try {
			const editsJson = JSON.stringify(store.toRenderState());
			await autosaveProject(documentPath, editsJson);
		} catch (err) {
			console.warn("Autosave failed:", err);
		}
	}, AUTOSAVE_INTERVAL_MS);
}

function stopAutosave() {
	if (autosaveTimer !== null) {
		clearInterval(autosaveTimer);
		autosaveTimer = null;
	}
}

// Tell the export store a panel-hosting editor is on screen. A fresh editor
// never has the panel open yet, so clear any stale foreground left by an
// "Open export" click from another route.
onMount(() => {
	exportActivity.setEditorPresent(true);
	exportActivity.minimize();
	// Re-adopt a still-running/queued export for THIS project so its panel (ring
	// or "Queued") can be reopened after navigating back.
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
		clearAutosave(documentPath).catch(() => {});
	}
	// Leave any in-flight/finished export in the store so it keeps tracking in
	// the activity center after navigation (the Rust process + global state
	// listener outlive this page). Just drop the foreground flag so the bell
	// shows it instead of assuming its panel is still on screen.
	exportActivity.minimize();
});

// Seek video + audio back to trimStart and resume. Used by both loop paths
// (timeupdate and ended); returns true so the timeupdate handler can bail.
function loopBackToStart(): boolean {
	if (!videoEl) return false;
	const start = store.trimStart || 0;
	videoEl.currentTime = start;
	for (const el of [systemAudioEl, micAudioEl]) {
		if (el) el.currentTime = start;
	}
	// WebCodecs path: the picture clock is the transport and the <video> stays
	// paused by design, so play()ing it just races that effect and rejects with
	// AbortError. Publishing the position is the whole handoff — VideoPreview
	// re-seats the clock onto it when we return true, and the audio engine
	// reschedules off the same backward jump.
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
		// Legacy <video> path only: in the WebCodecs path the clock owns time and
		// audio, so echoing this element's time would fight it across cuts.
		if (webcodecsActive) return;
		store.currentTime = videoEl.currentTime;
		// Loop only matters when trimEnd is below the natural duration; the
		// natural end uses the `ended` event (more precise than the ~250ms tick).
		if (loopEnabled && store.metadata) {
			const trimEnd = store.trimEnd > 0 ? store.trimEnd : store.metadata.duration;
			if (trimEnd > 0 && trimEnd < store.metadata.duration - 0.05) {
				if (videoEl.currentTime >= trimEnd - 0.05) {
					loopBackToStart();
					return;
				}
			}
		}
		// Drift correction: catch audio up when it falls behind the picture, but
		// never rewind it to chase a picture that stalled under load, because that
		// replays a slice as a live echo. Picture catch-up is owned by the rAF
		// sync loop (syncAudioToClock), so here we only nudge lagging audio.
		const videoT = videoEl.currentTime;
		for (const el of [systemAudioEl, micAudioEl]) {
			if (!el || el.paused) continue;
			const action = reconcileAvDrift({
				audioTime: el.currentTime,
				pictureTime: videoT,
				isJump: false,
				syncThreshold: 0.15,
				maxLead: AUDIO_MAX_LEAD,
			});
			if (action === "resync-audio") el.currentTime = videoT;
		}
	}
}

// Returns true when we looped, so the WebCodecs caller keeps its clock running
// instead of stopping. Loop wins over stop-at-end: the short-circuit avoids the
// pause calls below racing loopBackToStart.
function handleVideoEnded(): boolean {
	if (loopEnabled && videoEl) {
		return loopBackToStart();
	}
	store.isPlaying = false;
	systemAudioEl?.pause();
	micAudioEl?.pause();
	return false;
}

// Slave the audio (full-recording WAVs) to the cut-aware picture clock so they
// skip the same cuts. Normal playback stays locked at 1×; the only corrections
// are one snap per cut boundary and per seek. Audio that falls behind by more
// than this is nudged forward; audio that runs ahead of a stalled picture is
// NOT rewound (that replays a slice as a live echo). See reconcileAvDrift.
const AUDIO_SYNC_THRESHOLD = 0.12;
// A cut crossing or scrub jumps the playhead far past one publish quantum;
// detecting it snaps audio exactly on cuts of any length, including short ones.
const AUDIO_JUMP = 0.12;
// How far the audio may lead a stalled picture before we advance the PICTURE
// to catch up (a brief visual skip) instead of leaving the gap. Bounds the
// lip-sync drift that a decode stall under load would otherwise accumulate.
const AUDIO_MAX_LEAD = 0.5;
let audioSyncRaf: number | null = null;
let lastAudioTarget = -1;
function syncAudioToClock() {
	audioSyncRaf = requestAnimationFrame(syncAudioToClock);
	if (!store.isPlaying) {
		lastAudioTarget = -1;
		return;
	}
	// WebCodecs path: the gapless output clock owns time. Legacy <video> path:
	// the <video> element is the master and ALREADY skips cuts (it jumps to
	// cut.end at each boundary), so tracking it here makes the <audio> elements
	// skip the same cuts within a frame. Without this, the 4 Hz `timeupdate`
	// drift-check left audio playing the removed region for up to ~250 ms.
	const target = webcodecsActive ? store.currentTime : (videoEl?.currentTime ?? store.currentTime);
	const jumped = lastAudioTarget < 0 || Math.abs(target - lastAudioTarget) > AUDIO_JUMP;
	lastAudioTarget = target;
	for (const el of [systemAudioEl, micAudioEl]) {
		// CRITICAL: never stack a seek on an element that's still seeking (e.g.
		// cold-start buffering). Each new currentTime= interrupts the last, so it
		// never settles and the audio cuts out entirely. Wait for the current seek.
		if (!el || el.paused || el.seeking || el.readyState < 2) continue;
		// Snap on a cut/seek or when audio falls behind; when audio runs ahead of a
		// stalled picture, advance the picture rather than rewind audio (a rewind
		// replays a slice as a live echo, the record-while-previewing symptom).
		const action = reconcileAvDrift({
			audioTime: el.currentTime,
			pictureTime: target,
			isJump: jumped,
			syncThreshold: AUDIO_SYNC_THRESHOLD,
			maxLead: AUDIO_MAX_LEAD,
		});
		if (action === "resync-audio") {
			el.currentTime = target;
		} else if (action === "catch-picture" && videoEl) {
			videoEl.currentTime = el.currentTime;
		}
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
	// Bump first: an engine still decoding here would otherwise resolve into a
	// destroyed component and never be disposed.
	audioEngineGen++;
	audioEngine?.dispose();
});
onDestroy(disposeTileProvider);

// Kept audio regions and current OUTPUT time: what the Web Audio engine
// schedules against. Regions are the kept SEGMENTS (trim − cuts, split-bounded)
// each carrying its clip speed, so audio speeds up/down with the segment.
// Output time is the warped axis (store.timeMap), matching the picture clock.
function audioRegions() {
	return store.segments.map((s) => ({
		start: s.start,
		end: s.end,
		speed: store.segmentSpeedAt(s.start),
	}));
}
function outputNow() {
	return originalToOutput(store.timeMap, store.currentTime);
}
// Lazily build the engine on first WebCodecs playback. Tried once; on failure
// it's marked failed and the <audio> elements take over.
async function ensureAudioEngine() {
	if (audioEngine || audioEngineTried) return;
	audioEngineTried = true;
	if (!systemAudioSrc && !micAudioSrc) {
		audioEngineFailed = true;
		return;
	}
	const gen = audioEngineGen;
	try {
		const eng = await AudioTimelineEngine.create([
			{ url: systemAudioSrc, kind: "system" },
			{ url: micAudioSrc, kind: "mic" },
		]);
		// Decoding both tracks takes seconds on a long recording, and the file can
		// change or the editor close in that window. Adopting a stale engine
		// stranded its AudioContext — an OS audio thread — plus both fully decoded
		// PCM buffers, and left the new file early-returning on a truthy engine.
		if (gen !== audioEngineGen) {
			eng.dispose();
			return;
		}
		const s = store.audioSettings;
		// Detached: the recording's audio plays as voice clips, so the monolithic
		// source tracks are muted here (the clips path carries it).
		const detached = store.audioDetached;
		eng.setMasterVolume(s.volume, s.muted);
		eng.setTrackVolume("system", detached ? 0 : s.systemVolume, detached || s.systemMuted);
		eng.setTrackVolume("mic", detached ? 0 : s.micVolume, detached || s.micMuted);
		eng.setFades(s.fadeIn, s.fadeOut, store.timeMap.outputDuration);
		void eng.setMusicClips(buildMusicSpecs());
		audioEngine = eng;
	} catch (err) {
		console.warn("Web Audio engine unavailable; using <audio> fallback:", err);
		audioEngineFailed = true;
	}
}

// Play/pause audio in lockstep with `isPlaying`. WebCodecs path drives the
// Web Audio engine; the <audio> elements are the fallback / legacy path.
$effect(() => {
	const playing = store.isPlaying;
	const wc = webcodecsActive;
	const eng = audioEngine;
	const failed = audioEngineFailed;

	if (wc && !failed) {
		// Engine owns audio here: keep the <audio> elements and the seek loop off.
		for (const el of [systemAudioEl, micAudioEl]) el?.pause();
		stopAudioClockSync();
		if (playing) {
			void ensureAudioEngine();
			if (eng) {
				void eng.play(
					untrack(() => audioRegions()),
					untrack(() => outputNow()),
				);
			}
		} else {
			eng?.pause();
		}
		return;
	}

	// Fallback (engine failed) or legacy <video> path: slave the <audio>
	// elements to the playhead, and make sure the engine is silent.
	audioEngine?.pause();
	const alignTo = untrack(() => (wc ? store.currentTime : (videoEl?.currentTime ?? 0)));
	for (const el of [systemAudioEl, micAudioEl]) {
		if (!el) continue;
		if (playing) {
			el.currentTime = alignTo;
			void el.play().catch((err) => {
				console.warn("Audio play failed:", err);
			});
		} else {
			el.pause();
		}
	}
	// Run the rAF sync whenever the <audio> elements are the audio source:
	// both the legacy <video> path AND the engine-failed WebCodecs fallback.
	// It keeps them locked to the master (video time / output clock) so cuts
	// are skipped tightly, not just on the coarse `timeupdate` tick.
	if (playing) startAudioClockSync();
	else stopAudioClockSync();
});

// Reschedule the engine only on a seek/loop (output jump) or a kept-regions
// edit. Crossing a cut doesn't move gapless OUTPUT time, so it doesn't trigger.
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

// Legacy/fallback path: the <audio> elements are slaved to the <video> clock,
// so they must share its per-segment clip speed or audio plays at 1× while the
// picture speeds up. preservesPitch stays on (default), matching the export's
// pitch-preserving atempo. On the WebCodecs path these elements are paused
// (the Web Audio engine carries speed via the schedule), so this is a no-op.
$effect(() => {
	const segSpeed = store.segmentSpeedAtTime(store.currentTime);
	if (systemAudioEl) systemAudioEl.playbackRate = segSpeed;
	if (micAudioEl) micAudioEl.playbackRate = segSpeed;
});

// Apply volume/mute from the store's audio settings to both audio elements.
// The master is the product of the per-track gains so the user can keep
// system audio loud and mute just the mic, or vice versa. Master mute
// still zeros both.
$effect(() => {
	const settings = store.audioSettings;
	// Detached audio: the monolithic source tracks are silenced (voice clips
	// carry the recording audio); guards against double-playing the un-cut source.
	const detached = store.audioDetached;
	const systemVol =
		detached || settings.muted || settings.systemMuted
			? 0
			: Math.max(0, Math.min(1, (settings.volume * settings.systemVolume) / 10_000));
	const micVol =
		detached || settings.muted || settings.micMuted
			? 0
			: Math.max(0, Math.min(1, (settings.volume * settings.micVolume) / 10_000));
	if (systemAudioEl) systemAudioEl.volume = systemVol;
	if (micAudioEl) micAudioEl.volume = micVol;
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

// Re-decode/re-schedule music whenever the clip set changes (add/remove/edit).
// Reads the clips reactively; the engine dedupes decode work per call.
$effect(() => {
	const specs = buildMusicSpecs();
	void store.timeMap.outputDuration; // reschedule fill length on edit
	audioEngine?.setMusicClips(specs);
});

// Transport seek for `store.seek()`: seeks from outside the player (a
// transcript line, chapters, …). Most in-player seeks (timeline scrub,
// frame-step) already set `videoEl.currentTime` themselves; this gives panels
// the same reach. Moving the <video> works for both the legacy path and the
// WebCodecs path: paused → `seeked` realigns the picture clock; playing → the
// draw loop re-seats the clock off the changed `store.currentTime`. Setting
// `currentTime` alone failed mid-playback because the next time-publish (legacy)
// overwrote it before the seek took.
$effect(() => {
	const off = store.registerSeekHandler((t) => {
		if (videoEl) videoEl.currentTime = t;
		for (const el of [systemAudioEl, micAudioEl]) {
			if (el) el.currentTime = t;
		}
	});
	return off;
});

// Snap audio to the video time on scrub. Skipped on the WebCodecs path, where
// audio follows the clock and snapping to seeks would fight it.
function handleVideoSeeked() {
	if (!videoEl || webcodecsActive) return;
	const t = videoEl.currentTime;
	// Publish the jumped position immediately. During playback the <video>
	// cut-skip seeks to cut.end, but `store.currentTime` otherwise only catches
	// up on the next 4 Hz `timeupdate`, so captions/overlays (which key off
	// `store.currentTime`) lagged the cut by up to ~250 ms. Snap them here.
	store.currentTime = t;
	for (const el of [systemAudioEl, micAudioEl]) {
		if (el) el.currentTime = t;
	}
}

// Frame-step on the OUTPUT axis so stepping across a cut lands on the next
// kept frame, never inside a removed range. `store.currentTime` stays original.
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

// Build the WebCodecs filmstrip provider for the opened media. Tokened so a
// rapid reopen disposes a provider that resolves after we moved on.
async function setupTileProvider(url: string) {
	const token = ++tileProviderToken;
	disposeTileProvider();
	const dpr = browser ? window.devicePixelRatio || 1 : 1;
	const provider = await createTileProvider({
		url,
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
	// Skip without a usable duration: bumping the token would cancel an in-flight
	// strip, and a 0-duration source just yields black frames.
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
	// Warm the silence-detection cache off the back of the waveform pass (one
	// FFmpeg decode at a time, never on the load path). The result is discarded
	// here. `detectSilence` writes it to the file-identity cache the review
	// popover reads, so opening that popover is instant. Default options match
	// the popover's "balanced" sensitivity.
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

// Run after the editor is interactive, so heavy secondary work never competes
// with the preview's cold start. Same idle mechanism the waveform uses; fires
// at browser-idle or the timeout, whichever comes first.
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
	cameraSrc = "";
	videoEl?.pause();
	systemAudioEl?.pause();
	micAudioEl?.pause();
	// Tear down the previous file's engine; it rebuilds on first play. The bump
	// also disowns one still decoding, which `dispose()` alone cannot reach.
	audioEngineGen++;
	audioEngine?.dispose();
	audioEngine = null;
	audioEngineTried = false;
	audioEngineFailed = false;
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
		store.waveform = [];
		// Lazy: the idle-scheduled effect below extracts the waveform once the
		// editor is interactive, so the ffmpeg pass never competes with load.
		waveformRequested = false;
		systemAudioSrc = document.audioPath ? convertFileSrc(document.audioPath) : "";
		micAudioSrc = document.microphonePath ? convertFileSrc(document.microphonePath) : "";
		cameraPath = document.cameraPath ?? null;
		cameraSrc = cameraPath ? convertFileSrc(cameraPath) : "";
		// Mount the editor body (VideoPreview renders only when !isLoading) so the
		// <video> exists before load().
		isLoading = false;
		await tick();
		videoEl?.load();
		systemAudioEl?.load();
		micAudioEl?.load();
		// The preview now owns the main thread through its cold start. Defer the
		// three heavy secondary decoders — the Rust thumbnail strip, the filmstrip
		// tile decoder, and the cursor auto-zoom pass — to browser-idle. On a 4K
		// clip these each decode the same file; firing them alongside the preview
		// is what spiked open. The path guard drops them if a newer document opened
		// in the meantime.
		const loadedPath = document.projectPath;
		const filmstripSrc = videoSrc;
		runWhenIdle(() => {
			if (documentPath !== loadedPath) return;
			void loadThumbnailStrip(loadedPath);
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

// On first load, place a focus region at each detected click + settle. The
// `autoZoomApplied` document flag stops reopens from repopulating cleared regions.
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

async function runAutoZoom(opts: { silentEmpty?: boolean } = {}) {
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
	} finally {
		autoZoomRunning = false;
	}
}

// Re-run is exposed to FocusPanel via a window CustomEvent so the nested panel
// doesn't thread a prop through every component.
$effect(() => {
	function onRerun() {
		store.clearAutoZooms();
		store.autoZoomApplied = false;
		void runAutoZoom({ silentEmpty: false });
	}
	window.addEventListener("recast:rerun-auto-zoom", onRerun);
	return () => window.removeEventListener("recast:rerun-auto-zoom", onRerun);
});

// Export lifecycle UI. The exportActivity store owns the queue + run; this
// editor tracks the item it enqueued (myExportId) and maps it back to the
// values the export snippets read.
let myExportId = $state<string | null>(null);
const myItem = $derived(myExportId ? exportActivity.item(myExportId) : null);
// True while this editor rasterizes its edits (buildExportRenderState) before
// the item is enqueued: the frontend "preparing" window with its sub-steps.
let buildingExport = $state(false);
const isExportingHere = $derived(buildingExport || myItem?.status === "running");

let exportNow = $state<number>(Date.now());
const exportStartedAt = $derived(myItem?.startedAt ?? 0);
const exportCancelling = $derived(myItem?.phase === "cancelling");
const exportFinalizing = $derived(myItem?.phase === "finalizing");
const exportHasProgress = $derived((myItem?.progress ?? 0) > 0);
// Items ahead of this editor's queued export (the running one counts).
const queueAhead = $derived(myExportId ? exportActivity.queuePosition(myExportId) : 0);

let encodeMessageIndex = $state(0);

// Preparing-stage substages, surfaced in the dialog instead of a generic spinner.
let prepText = $state<"pending" | "running" | "done">("pending");
let prepCursor = $state<"pending" | "running" | "done">("pending");
let prepSending = $state<"pending" | "running" | "done">("pending");
function resetPrep() {
	prepText = "pending";
	prepCursor = "pending";
	prepSending = "pending";
}

// Eased display percentage: raw FFmpeg progress is jumpy, so lerp the ring
// toward it each animation tick while exporting.
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
	function tick(now: number) {
		const target = exportFinalizing ? 99.5 : Math.min(99.5, Math.max(0, myItem?.progress ?? 0));
		const dt = lastTs === null ? 16 : Math.max(1, Math.min(64, now - lastTs));
		lastTs = now;
		// Critically-damped follower (~250ms tau): ease-out toward target, no overshoot.
		const tau = 250;
		const k = 1 - Math.exp(-dt / tau);
		const next = displayPct + (target - displayPct) * k;
		// Never animate backwards; the export is monotonic so the ring should be too.
		displayPct = Math.max(displayPct, next);
		easeRafHandle = requestAnimationFrame(tick);
	}
	easeRafHandle = requestAnimationFrame(tick);
	return () => {
		if (easeRafHandle !== null) {
			cancelAnimationFrame(easeRafHandle);
			easeRafHandle = null;
		}
	};
});

function renderStateHasText(): boolean {
	return store.annotations.some((a) => a.kind.kind === "text");
}

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

// Terminal result of THIS editor's export, read from its queue item. The store
// owns the run + toasts + notification; the snippets just read this.
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

// Phase 4: browser-render the video + FFmpeg mux. OFF by default — flip to
// exercise the new path; any failure falls back to the classic Rust export.
const BROWSER_EXPORT_ENABLED = false;
async function handleExport() {
	if (isExportingHere) return;
	const exportId = createExportId();
	resetPrep();
	buildingExport = true;
	myExportId = exportId;
	exportActivity.show(exportId);
	exportNow = Date.now();

	try {
		// Build the payload Rust renders (text→PNG, cursor→sprite sheet); the
		// hooks drive the frontend "Preparing…" sub-stages.
		const { renderState: finalRenderState, metadata: meta } = await buildExportRenderState(store, {
			hooks: {
				onText: (s) => (prepText = s),
				onCursor: (s) => (prepCursor = s),
				onSending: (s) => (prepSending = s),
			},
		});

		// Warn (but don't block) if any image annotation can't be loaded. The
		// export skips them silently otherwise, shipping a video with them gone.
		const missingImages = await findMissingImageAnnotations(store);
		if (missingImages.length > 0) {
			const names = missingImages.map(basename).join(", ");
			toast.warning(
				`${missingImages.length} image${missingImages.length > 1 ? "s" : ""} couldn't be found and won't appear in the export: ${names}`,
			);
		}

		// A blur can't follow a zoom in the export, so warn so a redaction doesn't
		// silently slide off the thing it was covering.
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

		// Hand the fully-built export to the queue; the store runs it (after any
		// already-running one), so it survives leaving this editor.
		let browserVideoPath: string | undefined;
		const browserBlocked = browserExportBlockedReason(store);
		if (BROWSER_EXPORT_ENABLED && !browserBlocked) {
			try {
				browserVideoPath = await runBrowserExport(store, {
					videoUrl: videoSrc,
					cameraUrl: cameraSrc,
					quality: store.exportQuality as ExportQuality,
					fps: store.exportFps && store.exportFps > 0 ? store.exportFps : (meta?.fps ?? 30),
				});
			} catch (e) {
				console.error("browser export render failed; using the Rust compositor", e);
				browserVideoPath = undefined;
			}
		}

		exportActivity.enqueue({
			id: exportId,
			filename: data.filename,
			// Stable route path for identity/adoption; the actual media path is in
			// params.inputPath below.
			filePath: data.filePath,
			params: {
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
				browserVideoPath,
			},
		});
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

// Cancel this editor's export: the store stops a running one (or drops it from
// the queue if it hasn't started).
function handleCancelExport() {
	if (myExportId) void exportActivity.cancel(myExportId);
}

function dismissExportResult() {
	if (myExportId) exportActivity.dismiss(myExportId);
	myExportId = null;
	exportActivity.minimize();
}

// Watch the finished export in the in-app player. Opening it dismisses the
// export panel so the player isn't behind it. Size and created come from the
// exports listing (accurate); a minimal entry is the fallback so playback
// never hinges on the listing succeeding.
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

// Options phase is UI-only (the picker before Export); progress/result phases
// derive from the pipeline state, so the dialog is one surface that morphs.
let exportOptionsOpen = $state(false);
// The panel reflects only the export THIS editor enqueued (myItem), plus its
// own options picker. A queued item waits behind the running one.
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
// The panel is shown only when a phase is active AND it's foregrounded.
// Minimizing keeps the export alive but hands tracking to the activity center.
const isExportFlowOpen = $derived(exportPhase !== null && exportActivity.foreground);

// The control focus was on when the export flow opened, so we can hand focus
// back when it closes (the panel moves focus into itself on open). Without this,
// closing the panel strands focus on <body> and keyboard users lose their place.
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

// Drives the toolbar Export button: open the surface, close the picker,
// minimize a running/finished export to the activity center, or reopen it.
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
			// Remember where focus was (the Export button) so we can restore it when
			// the flow closes; the panel takes focus on open.
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

// Silence cuts only. Manual ripple deletes are always honoured, so they must
// not trip the "enable Silence detection" banner. Only auto cuts depend on it.
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

// Esc per phase: cancel a running export, dismiss a finished one, close the
// picker (which returns the timeline and properties panel).
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

async function revealExportInFolder() {
	if (exportResult?.kind !== "success") return;
	try {
		await openFileLocation(exportResult.path);
	} catch (err) {
		toast.error(`Could not open folder: ${err}`);
	}
}

// Push the latest export to Drive, or route to Settings to connect first
// (connecting opens a browser tab, which can't happen from this card).
async function uploadExportToDrive() {
	if (exportResult?.kind !== "success") return;
	await gdrive.init();
	if (!gdrive.connected) {
		toast.info("Connect Google Drive in Settings first.");
		void goto("/settings");
		return;
	}
	// Progress lives in the foreground dialog (and the activity center once
	// minimized), never in-place on the card. The store toasts the outcome.
	const id = gdrive.startUpload(exportResult.path);
	requestAnimationFrame(() => gdrive.setForeground(id));
}

// Share the export to Recast Cloud and copy the link; routes to Settings if
// not signed in. Progress surfaces through corner-notifications (phase-based).
async function shareCurrentExportToCloud() {
	if (exportResult?.kind !== "success") return;
	await cloudShare.init();
	if (!cloudShare.signedIn) {
		toast.info("Sign in to Recast Cloud in Settings first.");
		void goto("/settings");
		return;
	}
	const title = basename(exportResult.path)?.replace(/\.[^.]+$/, "") ?? "Recast";
	try {
		// The store surfaces success/error toasts and tracks progress in the
		// activity center, so just copy the link on success here.
		const result = await cloudShare.share(
			exportResult.path,
			title,
			undefined,
			buildCloudCaptionTranscript(store),
		);
		try {
			await navigator.clipboard.writeText(result.shareUrl);
		} catch {
			// Clipboard blocked; the link is still in the activity center.
		}
	} catch {
		// The store already surfaced the failure.
	}
}

// `navigator.share` exposure is static; sample once so the button renders
// without a reactive read.
const shareSupported = isShareSupported();

async function shareExportedFile() {
	if (exportResult?.kind !== "success") return;
	const fileName = basename(exportResult.path) ?? "recording";
	// OS share sheets can't attach a local file everywhere; fall back to a
	// recorded Drive link if this export already has one.
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
	// Paint the saving state before the synchronous serialize so the button
	// reflects the click immediately. The serialize itself stays on the main
	// thread by necessity, because Tauri's IPC bridge JSON-encodes command args on the
	// main thread anyway, so a worker would only add a proxy-stripping clone of
	// equal cost; the win is gating autosave on isDirty (see startAutosave).
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

// Bind the editor's mod-combo shortcuts to the central registry for the life
// of this route. Each bails while the export flow dialog owns the screen.
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

	// The export panel owns Esc routing while open; bail so global shortcuts
	// (play/pause, frame step) don't fire behind it.
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

	// Mod-combo shortcuts are owned by the central registry; bail on Ctrl/⌘ so a
	// combo never trips a plain-key action below.
	if (e.ctrlKey || e.metaKey) return;

	// Timeline editing commands (S/C/I/O/Home/End). These used to fire only when
	// the timeline scroller held DOM focus, so the keycaps in the toolbar lied
	// whenever focus sat anywhere else. Now they run at document scope, delegating
	// to the timeline's registered handlers (which own the frame math). Shift/Alt
	// variants stay scroller-local; a dropdown/popover open (isOverlayOpen) or a
	// collapsed timeline (no registered commands) makes them no-op.
	const runTimelineCommand = (run: (c: NonNullable<typeof store.timelineCommands>) => void) => {
		const c = store.timelineCommands;
		if (!c || !store.metadata || e.shiftKey || e.altKey || isOverlayOpen()) return;
		e.preventDefault();
		run(c);
	};

	// Plain keys: play/pause, frame step, fullscreen.
	switch (e.key) {
		case " ":
			// Buttons and links fire their click on Space KEYUP, so preventing the
			// keydown here would make every focused control in the editor dead to
			// the keyboard. The focused control wins; Space only reaches the
			// transport when nothing activatable holds focus.
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
		// Delete acts on the SELECTION, never on whatever holds DOM focus. It lives
		// here, at document scope, because the timeline, the zoom card and the
		// annotation overlay each used to claim it: Delete could remove the object
		// you weren't looking at, or two objects on one keypress.
		case "Delete":
		case "Backspace": {
			const removed = store.deleteSelection();
			if (!removed) return;
			e.preventDefault();
			// A clip delete closes the gap; park the playhead on the join so it lands
			// on a kept frame rather than inside the removed range.
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
			// Exit an armed tool first (the razor's emergency exit works from
			// anywhere now, not just when the scroller has focus), then deselect.
			// The annotation overlay cancels its own tool on Escape and
			// preventDefaults when it does, so we never fight it (bail on
			// defaultPrevented above).
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

// Extract the waveform lazily: defer to browser idle (best-effort) so the
// ffmpeg pass runs after the editor is interactive, never on the load path.
// The latch keeps the reactive re-runs from scheduling it more than once.
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

// Cycle the encode status messages while an export is running.
$effect(() => {
	if (myItem?.status !== "running") {
		encodeMessageIndex = 0;
		return;
	}
	const timer = setInterval(() => {
		encodeMessageIndex = (encodeMessageIndex + 1) % ENCODE_MESSAGES.length;
	}, 2600);
	return () => clearInterval(timer);
});

// Substages. During the frontend prep window (buildingExport) the rasterize
// sub-steps drive text/cursor/ship; once enqueued, the render state is built,
// so those are done and the single FFmpeg pass (which stitches cuts + overlays
// cursor/annotations/captions/zoom AND encodes) is the "Render frames" step.
const stages = $derived.by(() => {
	const prepFinished = !buildingExport;
	const running = myItem?.status === "running";
	const textState = prepFinished ? "done" : prepText;
	const cursorState = prepFinished ? "done" : prepCursor;
	const shipState = prepFinished ? "done" : prepSending;
	return [
		{
			key: "text" as const,
			label: "Render text overlays",
			state: textState,
			skip: textState === "done" && !renderStateHasText(),
		},
		{
			key: "cursor" as const,
			label: "Render cursor sprites",
			state: cursorState,
			skip: cursorState === "done" && store.cursorSettings.style === "dot",
		},
		{
			key: "ship" as const,
			label: "Hand off to encoder",
			state: shipState,
		},
		{
			key: "encode" as const,
			label: exportFinalizing ? "Finalise file" : "Render frames",
			state: (!prepFinished ? "pending" : running || exportFinalizing ? "running" : "pending") as
				| "pending"
				| "running"
				| "done",
		},
	];
});
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="fixed inset-0 flex min-h-screen w-full flex-col overflow-hidden bg-background text-foreground"
>
  <CustomTitlebar wrapperClass="h-9">
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
    <div class="flex min-h-0 flex-1 overflow-hidden">
      <!-- Preview + playback + timeline -->
      <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div
          bind:this={previewContainerEl}
          class="flex min-h-0 flex-1 flex-col items-center justify-center bg-background px-2 pt-1.5 pb-1"
        >
          <div
            class="flex-1 flex min-h-0 w-full items-center justify-center relative"
          >
            <VideoPreview
              {store}
              bind:videoEl
              bind:captureFrame
              bind:webcodecsActive
              {videoSrc}
              {cursorPath}
              {cameraSrc}
              onTimeUpdate={handleTimeUpdate}
              onEnded={handleVideoEnded}
              onLoadedMetadata={handleVideoLoadedMetadata}
              onReady={handleVideoReady}
              onError={handleVideoError}
              onSeeked={handleVideoSeeked}
              audioPositionSec={() => audioEngine?.positionOutputSec ?? null}
            />
          </div>
          <VideoPlayerControls
            {store}
            {videoEl}
            {captureFrame}
            bind:loopEnabled
            fullscreenTargetEl={previewContainerEl}
            showScrubber={!showTimeline}
          />
        </div>

        <!-- `slide` (axis:y) animates the wrapper height to 0 while the inner
             keeps its height, so the preview reclaims space smoothly. Timeline
             folds away in export mode so the preview owns the full height. -->
        {#if showTimeline && !isExportFlowOpen}
          <div
            class="shrink-0 overflow-hidden"
            transition:slide={{ axis: "y", duration: 280, easing: cubicOut }}
          >
            <Timeline {store} {videoEl} {tileProvider} {filmstripVersion} />
          </div>
        {/if}
      </div>

      <!-- Right rail. Editing shows the properties panel; entering export swaps
           it for the export surface. Both slide on the x-axis with the SAME
           duration/easing so the leaving and entering widths cancel to a
           monotonic reflow (no mid-swap wobble). The inner fixed-width div lets
           `slide` clip cleanly instead of reflowing container queries. -->
      {#if isExportFlowOpen}
        <aside
          class="min-h-0 shrink-0 overflow-hidden border-l border-border/60"
          transition:slide={{ axis: "x", duration: 280, easing: cubicOut }}
        >
          <div class="h-full w-[26rem]">
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
          </div>
        </aside>
      {:else if showSidebar}
        <aside
          class="relative min-h-0 shrink-0 overflow-hidden border-l border-border/60"
          transition:slide={{ axis: "x", duration: 280, easing: cubicOut }}
        >
          <!-- Splitter: drag or arrow-key to resize the panel. Sits in the left
               padding gutter so it never overlaps a tab. Modelled as a vertical
               slider (aria-valuenow = width), the same interactive-role idiom the
               timeline's trim/resize handles use. -->
          <div
            role="slider"
            tabindex="0"
            aria-orientation="vertical"
            aria-label="Resize properties panel"
            aria-valuemin={SIDEBAR_MIN}
            aria-valuemax={SIDEBAR_MAX}
            aria-valuenow={sidebarWidth}
            onpointerdown={startSidebarResize}
            onkeydown={onSidebarHandleKey}
            class="group absolute inset-y-0 left-0 z-20 w-1.5 cursor-col-resize focus-visible:outline-none"
          >
            <div
              class="mx-auto h-full w-px bg-border/50 transition-colors group-hover:bg-primary/60 group-focus-visible:bg-primary {resizingSidebar
                ? 'bg-primary!'
                : ''}"
            ></div>
          </div>
          <div class="h-full" style="width: {sidebarWidth}px;">
            <PropertiesPanel {store} {cameraPath} />
          </div>
        </aside>
      {/if}
    </div>
  {/if}

  <!-- .recast stores system + mic audio as separate WAVs (the mp4 has no audio);
       kept in lockstep with the video via the $effects above. -->
  <!-- preload="metadata": the Web Audio engine decodes the WAVs itself, so these
       fallback elements needn't buffer full PCM at open (~tens of MB each). -->
  {#if systemAudioSrc}
    <!-- svelte-ignore a11y_media_has_caption -->
    <audio
      bind:this={systemAudioEl}
      src={systemAudioSrc}
      preload="metadata"
      class="hidden"
    ></audio>
  {/if}
  {#if micAudioSrc}
    <!-- svelte-ignore a11y_media_has_caption -->
    <audio
      bind:this={micAudioEl}
      src={micAudioSrc}
      preload="metadata"
      class="hidden"
    ></audio>
  {/if}

  {#if playTarget}
    <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
  {/if}
</div>

{#snippet options()}
  <ExportDialog
    {store}
    onConfirm={confirmExportOptions}
    onCancel={dismissExportOptions}
  />
{/snippet}

{#snippet queued()}
  <div class="flex h-full min-h-0 flex-col">
    <header
      class="flex shrink-0 items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
      >
        <Clock class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
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
       anchored to "what you're exporting". -->
  <section
    class="flex items-stretch divide-x divide-border/40 border-b border-border/40 bg-muted/15 px-5 py-2.5"
  >
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 pr-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >Format</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {fmt.toUpperCase()}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 px-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >{isGifFmt ? "Colors" : "Quality"}</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {qualityLabel}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 px-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >FPS</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {fpsLabel}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 pl-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >Duration</span
      >
      <span class="truncate font-mono text-[12px] tabular-nums text-foreground">
        {formatClock(getExportDuration())}
      </span>
    </div>
  </section>
{/snippet}

{#snippet progress()}
  {@const isPreparing =
    !exportHasProgress && !exportFinalizing}
  {@const eta = exportEtaMs()}
  {@const ringPct = isPreparing
    ? 0
    : exportFinalizing
      ? 100
      : Math.min(100, Math.max(0, displayPct))}
  {@const RING_R = 52}

  <div class="flex h-full min-h-0 flex-col">
    <header
      class="flex shrink-0 items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
      >
        <Upload class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
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
      <div class="relative size-32" aria-live="polite">
        <svg
          viewBox="0 0 120 120"
          class="size-full -rotate-90 overflow-visible"
        >
          <!-- Track -->
          <circle
            cx="60"
            cy="60"
            r={RING_R}
            stroke="currentColor"
            stroke-width="6"
            class="fill-none text-muted"
          />
          {#if isPreparing}
                  <!-- Indeterminate spinner. `pathLength="100"` decouples the
                       dash math from 2πr so precision can't leave it short of full. -->
                  <circle
                    cx="60"
                    cy="60"
                    r={RING_R}
                    pathLength="100"
                    stroke="currentColor"
                    stroke-width="6"
                    stroke-linecap="round"
                    class="fill-none text-primary origin-center animate-spin"
                    style="stroke-dasharray: 25 100; animation-duration: 1.2s;"
                  />
                {:else}
                  <!-- Dash values in inline style so they participate in the CSS
                       transition; mixing attribute + style breaks it in some engines. -->
                  <circle
                    cx="60"
                    cy="60"
                    r={RING_R}
                    pathLength="100"
                    stroke="currentColor"
                    stroke-width="6"
                    stroke-linecap="round"
                    class="fill-none text-primary"
                    style="stroke-dasharray: 100; stroke-dashoffset: {100 - ringPct}; transition: stroke-dashoffset 220ms cubic-bezier(0.65, 0, 0.35, 1);"
                  />
                  {#if exportFinalizing}
                    <!-- Pulsing tip while we wait on FFmpeg's mux/move. -->
                    <circle
                      cx="60"
                      cy={60 - RING_R}
                      r="3.5"
                      class="fill-primary animate-pulse"
                    />
                  {/if}
                {/if}
              </svg>
              <!-- Percentage during encode; dashes while preparing/finalising. -->
              <div
                class="absolute inset-0 flex flex-col items-center justify-center"
              >
                {#if isPreparing}
                  <span
                    class="text-[11px] uppercase tracking-wider text-muted-foreground"
                    >Prep</span
                  >
                  <span class="text-[10px] text-muted-foreground">…</span>
                {:else if exportFinalizing}
                  <span
                    class="font-mono text-2xl font-semibold tabular-nums text-foreground"
                    >99%</span
                  >
                  <span
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                    >Finalising</span
                  >
                {:else}
                  <span
                    class="font-mono text-2xl font-semibold tabular-nums text-foreground"
                  >
                    {Math.floor(ringPct)}<span
                      class="text-base text-muted-foreground">%</span
                    >
                  </span>
                  {#if eta !== null}
                    <span
                      class="text-[10px] uppercase tracking-wider text-muted-foreground"
                      >~{formatElapsed(eta)} left</span
                    >
                  {:else if exportStartedAt}
                    <span
                      class="text-[10px] uppercase tracking-wider text-muted-foreground"
                      >{formatElapsed(exportNow - exportStartedAt)} elapsed</span
                    >
                  {/if}
                {/if}
              </div>
            </div>

            <!-- Rotating status line, shown only while frames are encoding. -->
            {#if !isPreparing && !exportFinalizing && !exportCancelling}
              <div class="relative h-4 self-stretch" aria-live="polite">
                {#key encodeMessageIndex}
                  <span
                    in:fade={{ duration: 320 }}
                    out:fade={{ duration: 320 }}
                    class="export-shimmer absolute inset-0 flex items-center justify-center text-[11px] font-medium tracking-tight"
                  >
                    {ENCODE_MESSAGES[encodeMessageIndex]}…
                  </span>
                {/key}
              </div>
            {/if}

            <!-- Substage stepper: done steps check off, the active step is the
                 highlighted row with a live pulsing dot, the rest stay dim. One
                 clear "which is happening now" language across all substages. -->
            <ul class="flex w-full flex-col gap-0.5 self-stretch text-[11px]">
              {#each stages as s (s.key)}
                {#if !s.skip}
                  {@const done = s.state === "done"}
                  {@const active = s.state === "running"}
                  <li
                    class="flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors duration-200 {active
                      ? 'bg-primary/5'
                      : ''}"
                  >
                    <span class="grid size-3.5 shrink-0 place-items-center">
                      {#if done}
                        <CheckCircle2 size={13} class="text-success" />
                      {:else if active}
                        <span
                          class="relative flex size-2.5 items-center justify-center"
                        >
                          <span
                            class="absolute inline-flex size-full rounded-full bg-primary/50 motion-safe:animate-ping"
                          ></span>
                          <span
                            class="relative inline-flex size-2 rounded-full bg-primary"
                          ></span>
                        </span>
                      {:else}
                        <span
                          class="size-2 rounded-full border border-muted-foreground/30"
                        ></span>
                      {/if}
                    </span>
                    <span
                      class="min-w-0 flex-1 truncate {active
                        ? 'font-medium text-foreground'
                        : done
                          ? 'text-muted-foreground'
                          : 'text-muted-foreground/50'}"
                    >
                      {s.label}{active ? "…" : ""}
                    </span>
                  </li>
                {/if}
              {/each}
            </ul>
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

{#snippet success()}
  <div class="flex h-full min-h-0 flex-col">
    <header class="flex shrink-0 items-start gap-3 border-b border-border/40 px-5 py-4">
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-success/30 bg-success/10 text-success shadow-(--shadow-craft-inset)"
      >
        <CheckCircle2 class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
        >
          Export complete
        </h3>
        {#if exportResult?.kind === "success"}
          <p
            class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground"
            title={exportResult.path}
          >
            {exportResult.path}
          </p>
        {/if}
      </div>
    </header>

    {@render exportSpecStrip()}

    <div class="min-h-0 flex-1 overflow-y-auto scrollbar-transparent">
    <!-- Share/upload tiles, grouped out of the footer so they read as one
         "where does this go?" choice. Upload progress is never shown inline
         here; it opens the foreground dialog and tracks in the activity center. -->
    <div class="border-t border-border/40 bg-muted/15 px-5 py-3.5">
      <p
        class="mb-2.5 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Share or upload
      </p>
      <div class="flex items-stretch gap-2">
        <button
          type="button"
          onclick={shareCurrentExportToCloud}
          class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
        >
          <span
            class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
          >
            <Cloud class="size-4" />
          </span>
          <span class="text-[11px] font-medium leading-none text-foreground">
            Recast Cloud
          </span>
        </button>

        <button
          type="button"
          onclick={uploadExportToDrive}
          class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
        >
          <span
            class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
          >
            <HardDriveUpload class="size-4" />
          </span>
          <span class="text-[11px] font-medium leading-none text-foreground">
            Google Drive
          </span>
        </button>

        {#if shareSupported}
          <button
            type="button"
            onclick={shareExportedFile}
            title="Open the system share sheet"
            class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
          >
            <span
              class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
            >
              <Share2 class="size-4" />
            </span>
            <span class="text-[11px] font-medium leading-none text-foreground">
              System share
            </span>
          </button>
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
        <Kbd class="ml-0.5">Esc</Kbd>
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
      class="flex shrink-0 items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-border/60 bg-muted text-muted-foreground shadow-(--shadow-craft-inset)"
      >
        <X class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
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
      class="flex shrink-0 items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-destructive/30 bg-destructive/10 text-destructive shadow-(--shadow-craft-inset)"
      >
        <TriangleAlert class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
        >
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
      <p
        class="mb-1.5 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Details
      </p>
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

<style>
  /* Primary-tinted highlight sweeps across muted text for a subtle shimmer. */
  .export-shimmer {
    background: linear-gradient(
      100deg,
      var(--muted-foreground) 0%,
      var(--muted-foreground) 38%,
      var(--primary) 50%,
      var(--muted-foreground) 62%,
      var(--muted-foreground) 100%
    );
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: export-shimmer-sweep 2.4s linear infinite;
  }
  @keyframes export-shimmer-sweep {
    from {
      background-position: 160% 0;
    }
    to {
      background-position: -160% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .export-shimmer {
      animation: none;
      background-position: 50% 0;
    }
  }
</style>
