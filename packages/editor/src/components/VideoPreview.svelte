<script lang="ts">
import { type MediaRef, mediaRefKey, textureRingFrames } from "@recast/media";
import type { MediabunnyVideoSource } from "@recast/media/playback";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { Spinner } from "@recast/ui/spinner";
import { onDestroy, onMount, untrack } from "svelte";
import { computeCanvasGeometry } from "../lib/canvas-geometry";
import { CursorSmoother } from "../lib/cursor/smoother";
import { smoothingStrengthToSigmaMs } from "../lib/cursor/smoothing";
import { getEditorServices } from "../lib/editor/services";
import { trackTimeAt } from "../lib/editor/track-offsets";
import { analytics, exportActivity } from "../lib/host-hooks";
import { AudioStallMonitor, resolveAvSync } from "../lib/playback/av-sync";
import { PlaybackClock } from "../lib/playback/clock";
import { loadCursorSprites } from "../lib/playback/cursor-sprites";
import { PreviewEngineDriver } from "../lib/playback/engine-driver";
import { createMediabunnySource } from "../lib/playback/mediabunny";
import { resolveCursorDataUrl } from "../lib/registry";
import { originalToOutput, outputToOriginal } from "../lib/timeline/time-map";
import { assetsStore } from "../stores/assets-store.svelte";
import { type EditorStore } from "../stores/editor-store.svelte";
import AnnotationOverlay from "./_components/AnnotationOverlay.svelte";
import CameraOverlay from "./_components/CameraOverlay.svelte";
import CaptionOverlay from "./_components/CaptionOverlay.svelte";
import FocusOverlay from "./_components/FocusOverlay.svelte";
import TextAnnotationLayer from "./_components/TextAnnotationLayer.svelte";
import { resolveBackgroundSrc } from "./background-source";
import { buildPressEvents, type PressEvent } from "./cursor-animation.logic";
import {
	type CursorSampleJS,
	classifyMbError,
	type IdlePeriodJS,
	resolutionTier,
	shouldRecoverMbSource,
} from "./video-preview.logic";

interface Props {
	store: EditorStore;
	videoEl: HTMLVideoElement | null;
	videoSrc: string;
	/** Same source as a ref. Decode streams off this; `videoSrc` only feeds the
	 *  `<video>` fallback element. Falls back to `videoSrc` when absent. */
	video?: MediaRef;
	cursorPath: string | null;
	/** convertFileSrc(camera.mp4) for this project, or empty when no
	 *  camera was recorded. Forwarded to CameraOverlay; the overlay
	 *  renders nothing when this is empty. */
	cameraSrc?: string;
	/** Milliseconds the camera track lags video frame 0 (measured at capture). */
	cameraOffsetMs?: number;
	onTimeUpdate: () => void;
	/** Return `true` if the host looped (moved the transport) instead of stopping. */
	onEnded: () => boolean | void;
	onLoadedMetadata: () => void;
	onReady: () => void;
	onError: () => void;
	onSeeked?: () => void;
	/** True once the WebCodecs preview engine is decoding for this source (so
	 *  the picture clock, not the `<video>` element, owns playback time).
	 *  The parent reads this to stop echoing `videoEl.currentTime` back into
	 *  `store.currentTime`, which otherwise fights the clock across cuts.
	 *  False whenever the legacy `<video>` path is active (flag off or the
	 *  source couldn't be demuxed/decoded). */
	webcodecsActive?: boolean;
	/** Exposed method that captures the current preview canvas as a PNG
	 *  blob (composite: video + background + zoom + blur + cursor, i.e.
	 *  WYSIWYG). Returns null if the WebGL context isn't ready or the
	 *  encode fails. Bind in the parent so other UI (player controls
	 *  copy-to-clipboard button) can trigger it. */
	captureFrame?: () => Promise<Blob | null>;
	/** Output-time position of the audio clock, or null when audio isn't
	 *  scheduled. The picture clock re-anchors onto it past the perceptual
	 *  drift threshold — two independent clocks otherwise separate. */
	audioPositionSec?: () => number | null;
}

let {
	store,
	videoEl = $bindable(null),
	videoSrc,
	video,
	cursorPath,
	cameraSrc = "",
	cameraOffsetMs = 0,
	onTimeUpdate,
	onEnded,
	onLoadedMetadata,
	onReady,
	onError,
	onSeeked,
	webcodecsActive = $bindable(false),
	captureFrame = $bindable(),
	audioPositionSec,
}: Props = $props();

let canvasEl: HTMLCanvasElement | null = $state(null);
let containerEl: HTMLDivElement | null = $state(null);
/** Shrink-wrap around the canvas so the annotation overlay can sit on top of
 * it at the same rendered rect regardless of letterboxing. */
let previewRectEl: HTMLDivElement | null = $state(null);
// Per-FRAME picture time for smooth DOM overlays (camera bubble). store.currentTime
// is throttled to ~25Hz to spare the timeline/waveform fan-out; the camera grow
// tracks the zoom curve, so it reads this instead to stay as smooth as the shader.
// null until the loop has drawn once: draw() early-returns while metadata or GL
// is missing, and pinning the overlays to a stale 0 is worse than letting them
// fall back to the <video> transport.
let smoothPreviewTime = $state<number | null>(null);
let isReady = $state(false);
/** The camera feed. The compositor draws the bubble, so the preview owns the
 *  element and hands it frames; the overlay is only its hit target. */
let cameraEl = $state<HTMLVideoElement | null>(null);
// Internal decoder that pre-decodes the first post-cut frame to mask the
// primary element's seek latency. Only seeked once per cut, never played.
let scoutEl = $state<HTMLVideoElement | null>(null);

let lastBgKey = "";
/// The Rust/wgpu compositor, the only renderer the preview has.
let engineDriver = $state<PreviewEngineDriver | null>(null);
let engineFailed = $state<string | null>(null);
// One line per session so a wrong backend or an empty composite is visible
// without turning on verbose logging.
let loggedEngineFrame = false;
let loggedFallbackFrameError = false;
// Rolling composite times for the engine path, reported once per window while
// playing. This is the second half of the WebGPU decision: whether it is
// available, and whether it holds the frame budget at this resolution.
let engineFrameMs: number[] = [];
let engineWindowStartedAt = 0;

/** WebGPU hides the adapter string from the page, so an empty name is the
 *  browser being careful rather than a failed probe. */
function describeAdapter(name: string): string {
	return name.trim() === "" ? "adapter hidden by the browser" : name;
}

function reportEngineFrameTimes(now: number) {
	if (engineFrameMs.length < 60 || now - engineWindowStartedAt < 5000) return;
	const sorted = [...engineFrameMs].sort((a, b) => a - b);
	const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))];
	const mean = sorted.reduce((sum, ms) => sum + ms, 0) / sorted.length;
	console.info(
		`preview engine composite over ${sorted.length} frames: ` +
			`mean ${mean.toFixed(2)}ms, p50 ${at(0.5).toFixed(2)}ms, ` +
			`p99 ${at(0.99).toFixed(2)}ms, max ${sorted[sorted.length - 1].toFixed(2)}ms`,
	);
	engineFrameMs = [];
	engineWindowStartedAt = now;
}
// Preview engine: `MediabunnyVideoSource` runs in a Web Worker and
// owns the MediaBunny Input + CanvasSink lifecycle. The composite samples
// a frame WE decode, not the <video> element's pixels, so jumping over a
// cut never waits on the native seek. The <video> element still drives
// the clock and audio sync (hybrid). When `create` fails (MediaBunny
// can't decode the file — see `unsupported-formats.ts` for the list),
// `mbSource` stays null and the draw loop falls back to the `<video>`
// element automatically. `mbSource` is not $state (read only from the
// imperative draw loop); `mbReady` is, because the markup and the
// pause-the-transport effect both branch on it.
let mbSource: MediabunnyVideoSource | null = null;
let mbReady = $state(false);
let loadedMbSrc = "";
// One user-facing notice per source when the hardware preview drops to the
// <video> fallback — otherwise a release failure is a silent blank screen.
let mbFallbackNotified = false;
function notifyPreviewFallback(reason: string) {
	if (mbFallbackNotified) return;
	mbFallbackNotified = true;
	toast.warning("Using the standard preview player", {
		description: `Hardware preview couldn't start for this video (${reason}). Playback works; scrubbing may be slower.`,
	});
}
// Automatic recovery from a transient decode failure — a GPU-process reset
// (TDR) under scrub-thrash kills the decoder + GL context but is recoverable;
// without this the preview degraded to <video> for the rest of the session.
const MB_RECOVER_DELAY_MS = 400;
let mbRecoverAttempts = 0;
let mbHealthyFrames = 0;
let mbRecoverTimer: ReturnType<typeof setTimeout> | undefined;
let mbRecoverNonce = $state(0);
// True once the engine has presented a frame. An early
// return from draw() clears to BLACK; we re-render the last frame instead, and
// this guards that.
let hasRenderedFrame = false;
/** Worst |video − audio| seen this session; reported with the perf sample. */
let maxAvDriftSec = 0;
const audioStall = new AudioStallMonitor();
let audioStalledReported = false;
// Last original time published to store.currentTime. Throttled because the write
// fans out to overlays/timeline/waveform; every-rAF writes starve frame delivery.
let lastPublishedTime = -1;
// Guards the end-of-timeline stop so it fires once per play session, not every
// frame while the clock sits clamped at the end. Reset when playback (re)starts.
let endHandled = false;

// Gapless OUTPUT-time clock that drives the PICTURE in the WebCodecs path. A
// <video> element's currentTime STALLS during its own seek, so borrowing it as
// the clock freezes the picture at every cut. This free-running integrator
// never stalls. Map output→original (outputToOriginal) for frame/cursor/zoom
// lookup; the <video> element stays the audio/seek transport and follows.
const picClock = new PlaybackClock();

// RAF handle for coalescing reactive redraws
let rafHandle: number | null = null;

// Cursor track
let cursorSamplesRaw: CursorSampleJS[] = [];
let cursorSamples: CursorSampleJS[] = []; // post-smoothing; read by interpolateCursor
// Off-thread smoother; results are applied async (see loadCursorTrackIfNeeded).
let smoother: CursorSmoother | null = null;
let idlePeriods: IdlePeriodJS[] = [];
let loadedCursorPath = "";

// Signature of the inputs that drive smoothing. Recomputing only when this
// changes keeps playback cheap even on long recordings.
let smoothingSignature = "";
/// Bumped on every write to `cursorSamples`. The engine keys its upload on this
/// rather than on the smoothing signature, which changes when smoothing is
/// requested rather than when the result lands.
let cursorVersion = 0;

let pressEvents: PressEvent[] = [];

/** Resize the engine's decoded-frame ring for a new source. */
function rebuildFrameRing(width: number, height: number) {
	engineDriver?.setScreenRingCapacity(textureRingFrames(width, height));
}

async function initEngine() {
	if (!canvasEl || engineDriver) return;
	try {
		engineDriver = await PreviewEngineDriver.create({ canvas: canvasEl });
		const info = engineDriver.info;
		analytics.capture("wasm_preview_init", { ...info });
		if (info.software) {
			toast.warning("Preview is running on a software GPU", {
				description: `No hardware adapter for ${info.backend}; playback will be slow.`,
			});
		}
		requestRedraw();
	} catch (err) {
		engineFailed = err instanceof Error ? err.message : String(err);
		console.error("preview engine failed to start:", err);
	}
}

//  Background loading
async function loadBackgroundIfNeeded() {
	if (!engineDriver) return;
	const type = store.backgroundType;
	const value = store.backgroundValue;
	// Including the resolved cache path in the key ensures the texture
	// re-loads when an `asset:<id>` download lands after an initial miss,
	// or when the thumbnail lands before the full-res does.
	let resolvedForKey = value;
	if (value.startsWith("asset:") && !value.startsWith("asset://")) {
		const id = value.slice("asset:".length);
		resolvedForKey = assetsStore.paths[id] ?? assetsStore.thumbPaths[id] ?? value;
	}
	const key = `${type}|${resolvedForKey}`;
	if (key === lastBgKey) return;
	lastBgKey = key;

	if (type !== "wallpaper" && type !== "image") {
		engineDriver.setBackgroundImage(null);
		return;
	}

	if (!value) {
		engineDriver.setBackgroundImage(null);
		return;
	}

	try {
		const resolvedSrc = await resolveBackgroundSrc(value);
		if (!resolvedSrc) {
			// Asset not yet cached (first-run offline, or still downloading).
			// Fall through to flat-background rendering until a later tick
			// re-runs this effect once the cache populates.
			return;
		}
		const img = new Image();
		img.crossOrigin = "anonymous";
		img.src = resolvedSrc;
		await img.decode();
		if (lastBgKey !== key) return; // Superseded by another load
		const bmp = await createImageBitmap(img);
		if (lastBgKey !== key) {
			bmp.close();
			return;
		}
		// Copied into a texture on the way in, so the bitmap is ours to close.
		engineDriver.setBackgroundImage(bmp);
		bmp.close();
		requestRedraw();
	} catch (err) {
		console.warn("Background image load failed:", err, "value:", value);
	}
}

//  Cursor track loading
async function loadCursorTrackIfNeeded() {
	if (!cursorPath || cursorPath === loadedCursorPath) return;
	try {
		const url = getEditorServices().resolveAssetUrl(cursorPath);
		const res = await fetch(url);
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		const json = (await res.json()) as {
			samples?: CursorSampleJS[];
			idlePeriods?: IdlePeriodJS[];
		};
		cursorSamplesRaw = json.samples ?? [];
		cursorVersion++;
		cursorSamples = cursorSamplesRaw;
		idlePeriods = json.idlePeriods ?? [];
		loadedCursorPath = cursorPath;
		smoothingSignature = "";
		// Publish raw samples for the Cursor panel's trajectory minimap.
		store.cursorSamplesRaw = cursorSamplesRaw;
		// Idle spans feed the browser export's idle-hide fade (parity with preview).
		store.cursorIdlePeriods = idlePeriods;
		// Press events come from raw samples, smoothing-independent.
		// Rebuild once per track load; the result is keyed by sample
		// timestamps, which never move regardless of smoothing settings.
		pressEvents = buildPressEvents(cursorSamplesRaw);
		if (!smoother) {
			smoother = new CursorSmoother((samples) => {
				cursorVersion++;
				cursorSamples = samples;
				requestRedraw();
			});
		}
		// By URL: the worker re-reads the track itself rather than us paying a
		// structured clone of ~225k sample objects on the main thread.
		smoother.load(cursorSamplesRaw, url);
		ensureSmoothingCurrent();
	} catch (err) {
		console.warn("Cursor track load failed:", err);
		cursorSamplesRaw = [];
		cursorVersion++;
		cursorSamples = [];
		idlePeriods = [];
		pressEvents = [];
	}
}

// Recompute the smoothed cursor path whenever the inputs change. Called once
// per draw(): cheap signature check, real work only on deltas. The signature
// is set immediately (in-flight marker) so the per-frame call doesn't re-fire
// the request while the worker runs; the result is applied async via the
// smoother's callback. `sigmaMs <= 0` is the raw path, applied inline since
// there's nothing to compute.
function ensureSmoothingCurrent() {
	if (cursorSamplesRaw.length === 0) {
		cursorVersion++;
		cursorSamples = cursorSamplesRaw;
		smoothingSignature = "";
		return;
	}
	const cs = store.cursorSettings;
	const sig = `${loadedCursorPath}|${cs.smoothing}|${cs.snapToClicks ? 1 : 0}|${cs.snapWindowMs}`;
	if (sig === smoothingSignature) return;
	smoothingSignature = sig;
	const sigmaMs = smoothingStrengthToSigmaMs(cs.smoothing);
	if (sigmaMs <= 0) {
		cursorVersion++;
		cursorSamples = cursorSamplesRaw;
		requestRedraw();
		return;
	}
	smoother?.request({
		sigmaMs,
		snapToClicks: cs.snapToClicks,
		snapWindowMs: cs.snapWindowMs,
	});
}

//  Per-frame memoization
// The draw loop runs at 60fps during playback; these recompute-on-change
// caches stop it re-parsing/re-allocating identical values every frame.
let geomCache: ReturnType<typeof computeCanvasGeometry> | null = null;
let geomSig = "";
function currentGeometry() {
	const meta = store.metadata;
	if (!meta?.width || !meta?.height) return null;
	const sig = `${meta.width}x${meta.height}|${store.padding}|${store.outputAspect}`;
	if (sig !== geomSig) {
		geomCache = computeCanvasGeometry(meta.width, meta.height, store.padding, store.outputAspect);
		geomSig = sig;
	}
	return geomCache;
}

// Container CSS size, cached by the ResizeObserver so the draw loop never
// reads clientWidth/clientHeight — a forced synchronous reflow every frame,
// made worse by the overlays that dirty layout on the same tick.
let containerW = 0;
let containerH = 0;

//  Sizing
function resizeCanvas() {
	if (!canvasEl || !containerEl) return false;
	const geom = currentGeometry();
	if (!geom) return false;
	const compW = geom.canvasW;
	const compH = geom.canvasH;

	const cw = containerW || containerEl.clientWidth;
	const ch = containerH || containerEl.clientHeight;
	if (cw <= 0 || ch <= 0) return false;

	// Fit composition into container preserving aspect
	const scale = Math.min(cw / compW, ch / compH);
	const cssW = Math.max(1, Math.floor(compW * scale));
	const cssH = Math.max(1, Math.floor(compH * scale));

	// Render at devicePixelRatio for crispness, capped at the composition's
	// native resolution (no point upscaling) and at 2160p to bound GPU cost.
	const dpr = Math.min(window.devicePixelRatio || 1, 2);
	const maxDim = 2160;
	let bufW = Math.min(Math.round(cssW * dpr), compW, maxDim);
	let bufH = Math.min(Math.round(cssH * dpr), compH, maxDim);
	// Maintain aspect after caps
	const bufScale = Math.min(bufW / compW, bufH / compH);
	bufW = Math.max(1, Math.floor(compW * bufScale));
	bufH = Math.max(1, Math.floor(compH * bufScale));

	canvasEl.style.width = `${cssW}px`;
	canvasEl.style.height = `${cssH}px`;
	if (canvasEl.width !== bufW || canvasEl.height !== bufH) {
		canvasEl.width = bufW;
		canvasEl.height = bufH;
	}
	return true;
}

//  Render
// Target time of an in-flight cut-skip seek. draw() issues each skip ONCE
// rather than re-assigning currentTime every frame while the decoder is
// still seeking (which thrashes it into a multi-second stall).
let cutSkipTarget: number | null = null;
// Time the scout element is currently seeking/seeked to, so we don't
// re-issue the same pre-decode seek every frame.
let scoutSeekTarget: number | null = null;

// How early (s) to start pre-decoding the post-cut frame on the SCOUT
// element, and how early to actually jump the primary. The scout window is
// larger so the post-cut frame is decoded and ready by the time we reach
// the boundary, so that decoded frame masks the primary's seek latency.
const SCOUT_PRESEEK_LOOKAHEAD = 0.6;
const CUT_JUMP_LOOKAHEAD = 0.12;
// WebCodecs cross-cut decode-ahead: how far ahead (in OUTPUT seconds) of an
// upcoming cut to start warming the post-cut GOP on the worker's scout
// decoder, so crossing the cut doesn't freeze while the primary re-decodes
// from a keyframe. Wants to cover the post-cut GOP's decode time; ~1s GOP
// recordings are well covered, longer-GOP legacy files are partially helped.
const WC_PREFETCH_LOOKAHEAD = 2.0;
// How close the scout's landed time must be to the cut end to treat its
// frame as a valid stand-in (a seek may land a frame or two off target).
const SCOUT_READY_EPS = 0.1;

/** True when the scout has the post-cut frame decoded and ready to sample. */
function scoutReadyAt(t: number): boolean {
	return (
		!!scoutEl &&
		!scoutEl.seeking &&
		scoutEl.readyState >= 2 &&
		scoutEl.videoWidth > 0 &&
		Math.abs(scoutEl.currentTime - t) < SCOUT_READY_EPS
	);
}

function draw() {
	if (!canvasEl || !store.metadata) return;
	if (!engineDriver) return;
	if (!resizeCanvas()) return;

	// Refresh the smoothed cursor path if any of its inputs changed since
	// the last frame. Signature-based guard keeps this effectively free
	// (one string compare) when nothing's changed.
	ensureSmoothingCurrent();

	// Picture time. WebCodecs path: the gapless OUTPUT clock is master
	// (output→original feeds frame/cursor/zoom); the <video>/audio transport
	// follows but is never read for the picture, so its seek stalls can't
	// freeze playback. Legacy path: the <video> currentTime is the clock.
	const usingPicClock = mbReady;
	let playbackTime: number;
	if (usingPicClock && store.isPlaying) {
		// External scrub while playing: the timeline/controls set
		// store.currentTime to a value we didn't publish ourselves. Re-seat the
		// clock onto it so seeking works mid-playback instead of snapping back.
		// (We compare against our own last publish, so this never fires for the
		// values WE wrote.)
		if (Math.abs(store.currentTime - lastPublishedTime) > 0.05) {
			picClock.seek(originalToOutput(store.timeMap, store.currentTime));
			lastPublishedTime = store.currentTime;
			endHandled = false;
		}
		// Audio runs on the sound card's clock, the picture on wall time. Pull
		// the picture back onto audio once the gap is perceptible.
		const audioTime = audioPositionSec?.() ?? null;
		const sync = resolveAvSync({
			videoTime: picClock.time,
			audioTime,
			playing: true,
			audioStalledSec: audioStall.observe(audioTime, true, performance.now()),
		});
		maxAvDriftSec = Math.max(maxAvDriftSec, Math.abs(sync.driftSec));
		if (sync.resync) picClock.seek(sync.target);
		// Report once per stall, not once per frame.
		if (sync.audioStalled !== audioStalledReported) {
			audioStalledReported = sync.audioStalled;
			if (sync.audioStalled) console.warn("Audio clock stalled; picture running unmastered");
		}
		// Playing: the gapless output clock is the master.
		playbackTime = outputToOriginal(store.timeMap, picClock.time);
		// Reached the end of the edited timeline. Ask the host BEFORE stopping:
		// it may want to loop, and stopping first would flip isPlaying
		// false→true within one tick, which Svelte batches into no change at
		// all — the play/pause effect never re-seeds, so the clock stays
		// clamped at the end and the picture sticks on the last frame.
		if (picClock.atEnd && !endHandled) {
			if (onEnded?.() === true) {
				// The host moved the transport; follow it and keep playing.
				picClock.seek(originalToOutput(store.timeMap, store.currentTime));
				lastPublishedTime = store.currentTime;
			} else {
				// The clock clamps at its duration, so without this the picture
				// would freeze on the last frame while still "playing" (and the
				// decoder would sit idle). Hitting play again restarts from the
				// top (see the seed below).
				endHandled = true;
				store.isPlaying = false;
			}
		}
		// Publish to the store (drives overlays/timeline/audio) at ~25 Hz, not
		// every rAF frame, because that fan-out is expensive and was starving decoded-
		// frame delivery. Always publish on a backward step or a jump so cuts
		// and seeks stay exact.
		if (playbackTime >= lastPublishedTime + 0.04 || playbackTime < lastPublishedTime) {
			store.currentTime = playbackTime;
			lastPublishedTime = playbackTime;
		}
		// Keep the <video> transport roughly aligned so the legacy fallback can
		// take over mid-playback. It stays paused here (see the effect below),
		// so this is a cheap single-frame seek, not continuous decode.
		if (videoEl && !videoEl.seeking && Math.abs(videoEl.currentTime - playbackTime) > 0.25) {
			videoEl.currentTime = playbackTime;
		}
	} else if (usingPicClock) {
		// Paused on the MediaBunny path: the store owns the time. Reading the
		// <video> here would tie us to an element we keep paused (it must not
		// decode in parallel with the worker), whose currentTime only tracks
		// within the 0.25s alignment tolerance below.
		playbackTime = store.currentTime;
	} else {
		// Legacy path: the <video> transport owns the time, so a scrub or
		// frame-step sets it directly. handleSeeked realigns the picture
		// clock so resuming continues from here.
		playbackTime = videoEl ? videoEl.currentTime : store.currentTime;
		// Publish from this rAF loop, NOT from the element's `timeupdate`: that
		// event fires on a ~250ms tick, so everything reading store.currentTime
		// (the scrubber, the playhead, overlays) advanced in visible ~4Hz steps.
		// Same ~25Hz throttle as the WebCodecs branch above, and only while
		// playing — paused, the store owns the position and echoing the element
		// back would fight a scrub.
		if (
			store.isPlaying &&
			(playbackTime >= lastPublishedTime + 0.04 || playbackTime < lastPublishedTime)
		) {
			store.currentTime = playbackTime;
			lastPublishedTime = playbackTime;
		}
	}

	// Publish the per-frame clock for smooth overlays (unthrottled, unlike the
	// store fan-out above). One number write; only the camera bubble reads it.
	smoothPreviewTime = playbackTime;

	// Legacy-path cut skip: two decoders leapfrog the removed gap.
	//   1. As the playhead nears a cut, the SCOUT pre-decodes the first
	//      post-cut frame (cut.end), well ahead of the boundary.
	//   2. At the boundary the PRIMARY jumps to cut.end (keeps store time &
	//      audio correct); while it settles we sample the scout's already-
	//      decoded frame, so there's no visible freeze. Both land on the same
	//      time/content, so the swap is seamless.
	// Seek issued ONCE per cut: re-assigning currentTime mid-seek thrashes the
	// decoder into a multi-second stall. Scrubbing into a cut stays allowed
	// (gated on isPlaying); `cutsEnabled` off bypasses it.
	let frameEl: HTMLVideoElement | null = videoEl;
	const activeCuts = store.effectiveCuts;
	// Legacy <video> cut-skip (scout + primary seek). OFF for the WebCodecs
	// path: its output clock is gapless, so there's no gap to skip. Crossing a
	// cut is just the scheduler resetting to the post-cut GOP, and the frame
	// selector holds (never steps back) until that GOP decodes. Critically we
	// must NOT decode through the removed region, which would flood the decoder.
	if (!mbReady && videoEl && store.isPlaying && activeCuts.length > 0) {
		const cut = activeCuts.find(
			(c) => playbackTime + SCOUT_PRESEEK_LOOKAHEAD >= c.start && playbackTime < c.end - 0.02,
		);
		if (cut) {
			// (1) Pre-decode the post-cut frame on the scout, ahead of the jump.
			if (scoutEl && scoutSeekTarget !== cut.end) {
				scoutSeekTarget = cut.end;
				try {
					scoutEl.currentTime = cut.end;
				} catch {
					/* scout not ready to seek yet; retried next frame */
				}
			}
			// (2) At the boundary, jump the primary and mask its seek with the
			//     scout's pre-decoded frame.
			if (playbackTime + CUT_JUMP_LOOKAHEAD >= cut.start) {
				if (cutSkipTarget !== cut.end && !videoEl.seeking) {
					cutSkipTarget = cut.end;
					videoEl.currentTime = cut.end;
				}
				if (scoutReadyAt(cut.end)) {
					// Draw the scout's frame this tick, no visible freeze.
					frameEl = scoutEl;
				} else {
					// Scout not ready (e.g. sparse keyframes): hold the last
					// frame until the primary settles, as before.
					return;
				}
			} else {
				// Approaching but not yet at the boundary: keep playing the
				// primary normally; the jump hasn't happened.
				cutSkipTarget = null;
			}
		} else {
			// Outside any cut window: clear so the next cut can fire.
			cutSkipTarget = null;
			scoutSeekTarget = null;
		}
	}

	// Cross-cut decode-ahead: if playback will cross a cut within the lookahead
	// window, warm the post-cut GOP on the worker's scout decoder NOW so the
	// crossing is seamless instead of freezing while the primary re-decodes
	// from a keyframe. Output time is gapless, so we look ahead in OUTPUT time
	// and map back to original to find the next cut we'll reach. Issued every
	// frame while approaching; the worker dedupes per post-cut GOP.
	if (usingPicClock && store.isPlaying && mbSource && mbReady && activeCuts.length > 0) {
		const lookaheadOrig = outputToOriginal(store.timeMap, picClock.time + WC_PREFETCH_LOOKAHEAD);
		const upcoming = activeCuts.find((c) => c.start > playbackTime && c.start <= lookaheadOrig);
		if (upcoming) mbSource.prefetch(upcoming.end);
	}

	{
		const engineStartedAt = performance.now();
		// The engine evaluates the scene itself, so it takes OUTPUT time; the
		// original-axis `playbackTime` is only used to pick a decoded frame.
		const outputTime = usingPicClock
			? picClock.time
			: originalToOutput(store.timeMap, playbackTime);
		engineDriver.setCanvasSize(canvasEl.width, canvasEl.height);
		syncEngineFrameInputs();

		let bound = false;
		if (mbSource && mbReady) {
			let floorSec = 0;
			for (const c of activeCuts) if (c.end <= playbackTime && c.end > floorSec) floorSec = c.end;
			mbSource.advanceTo(Math.max(0, playbackTime));
			bound = engineDriver.bindScreenFrame(
				Math.max(0, Math.round(playbackTime * 1e6)),
				Math.max(0, Math.round(floorSec * 1e6)),
			);
		} else if (frameEl && frameEl.readyState >= 2 && frameEl.videoWidth > 0) {
			// `<video>` fallback: there is no decode stream to ring, so the frame
			// is uploaded and bound in the same tick.
			const tUs = Math.max(0, Math.round(playbackTime * 1e6));
			let frame: VideoFrame | null = null;
			try {
				frame = new VideoFrame(frameEl, { timestamp: tUs });
				engineDriver.putScreenFrame(frame, tUs);
				bound = engineDriver.bindScreenFrame(tUs, 0);
			} catch (err) {
				// The element reports readyState 2 before it holds a decodable
				// frame, so this fires on every source change. Once per source is
				// enough to notice a real failure.
				if (!loggedFallbackFrameError) {
					loggedFallbackFrameError = true;
					console.warn("preview engine could not take the fallback frame:", err);
				}
			} finally {
				frame?.close();
			}
		}
		putCameraFrame(Math.max(0, Math.round(playbackTime * 1e6)));
		if (!bound && !hasRenderedFrame) return;

		try {
			const drawn = engineDriver.render(outputTime);
			if (!loggedEngineFrame) {
				loggedEngineFrame = true;
				engineWindowStartedAt = performance.now();
				const info = engineDriver.info;
				console.info(
					`preview engine: ${info.backend} on ${describeAdapter(info.adapter)}` +
						`${info.software ? " (software)" : ""}, ${drawn} layer(s) drawn`,
				);
			}
		} catch (err) {
			engineFailed = err instanceof Error ? err.message : String(err);
			console.error("preview engine render failed:", err);
			return;
		}
		hasRenderedFrame = true;
		if (!isReady) isReady = true;
		const finishedAt = performance.now();
		engineFrameMs.push(finishedAt - engineStartedAt);
		reportEngineFrameTimes(finishedAt);
	}
}

function requestRedraw() {
	// While playing, `startVideoFrameLoop` already draws every rAF. This handle is
	// separate from `wcRafHandle`, so without this guard the ~25Hz `currentTime`
	// publish from inside draw() re-entered here and scheduled a SECOND full
	// composite — ~85 draws/sec instead of 60. `stopVideoFrameLoop` paints once on
	// the way out so a change made mid-playback isn't stranded.
	if (wcRafHandle !== null) return;
	if (rafHandle !== null) return;
	rafHandle = requestAnimationFrame(() => {
		rafHandle = null;
		try {
			draw();
			if (paintFailed) paintFailed = false;
		} catch (err) {
			// Paused, a throw here leaves the last good frame on screen, so every
			// later edit silently appears to do nothing. Say so instead.
			console.error("preview draw() failed:", err);
			paintFailed = true;
		}
	});
}

//  Playback frame loop (rAF)
// rAF handle for the preview playback loop (see startVideoFrameLoop).
let wcRafHandle: number | null = null;
// Consecutive draw() failures; a bad frame must not kill the loop.
let drawErrors = 0;
// The composite is stuck on a stale frame; surfaced in the template.
let paintFailed = $state(false);

function startVideoFrameLoop() {
	// Drive the loop with rAF, not the <video> element's requestVideoFrameCallback:
	// rVFC fires only when the element presents a new frame, which STALLS during
	// the seek we issue at a cut, the very moment we must keep painting. draw()
	// reads the master clock (the WebCodecs picture clock when active, else
	// videoEl.currentTime), so an rAF loop stays smooth across cuts on both the
	// WebCodecs path and the <video> fallback.
	if (wcRafHandle !== null) return;
	const loop = () => {
		try {
			draw();
			drawErrors = 0;
			if (paintFailed) paintFailed = false;
		} catch (err) {
			// A bad frame must not kill the loop (a dead loop freezes the preview
			// and reads as a crash). Log once, tolerate transients, stop if persistent.
			if (drawErrors++ === 0) console.error("preview draw() failed:", err);
			if (drawErrors > 120) {
				console.error("preview draw() failing persistently; stopping loop");
				wcRafHandle = null;
				paintFailed = true;
				return;
			}
		}
		wcRafHandle = requestAnimationFrame(loop);
	};
	wcRafHandle = requestAnimationFrame(loop);
}

function stopVideoFrameLoop() {
	if (wcRafHandle !== null) {
		cancelAnimationFrame(wcRafHandle);
		wcRafHandle = null;
		// Property changes during playback were swallowed by the guard in
		// `requestRedraw`; paint once now so the paused frame is current.
		requestRedraw();
	}
}

/**
 * Capture the current preview frame as a PNG blob: the full composite
 * (video + background + zoom + blur + cursor), so the screenshot matches
 * what the user sees rather than the raw recording.
 *
 * Copied through a 2D canvas because `toBlob` on the engine's own surface is
 * not guaranteed to see the presented frame, while an inter-canvas `drawImage`
 * is.
 */
$effect(() => {
	captureFrame = async () => {
		if (!canvasEl || !engineDriver || !hasRenderedFrame) return null;
		try {
			const w = canvasEl.width;
			const h = canvasEl.height;
			if (!w || !h) return null;
			const copy = document.createElement("canvas");
			copy.width = w;
			copy.height = h;
			const ctx = copy.getContext("2d");
			if (!ctx) return null;
			ctx.drawImage(canvasEl, 0, 0);
			return await new Promise<Blob | null>((resolve) => {
				copy.toBlob((b) => resolve(b), "image/png");
			});
		} catch (err) {
			console.warn("captureFrame failed", err);
			return null;
		}
	};
});

/** Playback we suspended on hide, to restore on show. */
let resumeOnVisible = false;

// devicePixelRatio has no change event, and ResizeObserver stays silent when
// only the scale factor changes — dragging the window to a monitor at a
// different DPI while paused would otherwise leave the buffer at the old DPR.
let dprQuery: MediaQueryList | null = null;
function watchDpr() {
	dprQuery?.removeEventListener("change", onDprChange);
	dprQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
	dprQuery.addEventListener("change", onDprChange);
}
function onDprChange() {
	watchDpr();
	requestRedraw();
}

/**
 * rAF stops when the window is hidden, but the picture clock and the audio
 * graph keep running — so playback would advance with no decoding behind it,
 * then jump on return. Cut-skipping and the end-of-timeline check also live
 * in the draw loop, so both would be missed entirely.
 */
function onVisibilityChange() {
	if (document.hidden) {
		resumeOnVisible = store.isPlaying;
		if (resumeOnVisible) store.isPlaying = false;
	} else if (resumeOnVisible) {
		resumeOnVisible = false;
		store.isPlaying = true;
	}
}

// Engine scene sync. Reads the whole render state, so this effect re-runs on
// any store write; the driver drops an unchanged scene rather than rebuilding
// the evaluator for nothing.
$effect(() => {
	if (!engineDriver) return;
	const state = store.toRenderState();
	const meta = store.metadata;
	const timeMap = store.timeMap;
	untrack(() => {
		if (!engineDriver) return;
		if (meta?.width && meta?.height) engineDriver.setSourceSize(meta.width, meta.height);
		// Before the scene: the axis is what output time MEANS, and a scene
		// carrying a cut this map drops would otherwise resolve one frame on the
		// old axis.
		engineDriver.setTimeMap(timeMap);
		engineDriver.syncScene(state);
		requestRedraw();
	});
});

/** Version of the cursor track last handed to the engine. Stringifying a
 *  225-second track every frame would cost more than the composite. */
let engineCursorSignature = "";

/** One slot, uploaded and bound in the same tick: the element is a seek-only
 *  transport, so there is no decode stream to buffer. */
function putCameraFrame(timestampUs: number) {
	if (!engineDriver || !cameraEl) return;
	if (cameraEl.readyState < 2 || cameraEl.videoWidth === 0) return;
	let frame: VideoFrame | null = null;
	try {
		frame = new VideoFrame(cameraEl, { timestamp: timestampUs });
		engineDriver.putCameraFrame(frame, timestampUs);
	} catch (err) {
		if (!loggedCameraFrameError) {
			loggedCameraFrameError = true;
			console.warn("preview engine could not take the camera frame:", err);
		}
	} finally {
		frame?.close();
	}
}
let loggedCameraFrameError = false;

function syncEngineFrameInputs() {
	if (!engineDriver) return;
	if (mbSource) {
		engineDriver.setScreenRingCapacity(textureRingFrames(mbSource.width, mbSource.height));
	}
	if (engineCursorSignature !== String(cursorVersion)) {
		engineCursorSignature = String(cursorVersion);
		engineDriver.setCursorTrack(
			cursorSamples.length > 0 ? { samples: cursorSamples, idlePeriods } : null,
		);
	}
}

/** Paths last handed to the engine, so a store write that leaves the image
 *  annotations alone does not re-decode every asset. */
let engineImageKey = "";

// Assets for image annotations. The compositor draws them, so it needs the
// decoded bitmap; a path that fails to decode simply never uploads and the
// annotation is skipped rather than drawn as a placeholder.
$effect(() => {
	if (!engineDriver) return;
	const paths = [
		...new Set(
			store.annotations
				.filter((a) => a.kind.kind === "image" && a.kind.path)
				.map((a) => (a.kind as { path: string }).path),
		),
	].sort();
	const key = paths.join("\u0000");
	untrack(() => {
		if (!engineDriver || key === engineImageKey) return;
		engineImageKey = key;
		void loadAnnotationImages(paths, key);
	});
});

async function loadAnnotationImages(paths: string[], key: string) {
	const images = new Map<string, ImageBitmap>();
	for (const path of paths) {
		try {
			const img = new Image();
			img.crossOrigin = "anonymous";
			img.src = getEditorServices().resolveAssetUrl(path);
			await img.decode();
			images.set(path, await createImageBitmap(img));
		} catch (err) {
			console.warn("annotation image could not be decoded:", path, err);
		}
	}
	// Superseded while decoding: the newer set owns the engine now.
	if (engineImageKey !== key) {
		for (const image of images.values()) image.close();
		return;
	}
	if (engineDriver?.setAnnotationImages(key, images)) requestRedraw();
	for (const image of images.values()) image.close();
}

// Pointer sprites for the engine path. A style with no sprite uploads nothing,
// which is what leaves the engine drawing its dot.
$effect(() => {
	if (!engineDriver) return;
	const style = store.cursorSettings.style;
	untrack(() => {
		void loadCursorSprites(style, resolveCursorDataUrl).then((sprites) => {
			if (engineDriver?.setCursorSprites(style, sprites)) requestRedraw();
		});
	});
});

// The camera recorder starts at its own instant, so `cameraOffsetMs` maps
// between the two tracks. Tolerance avoids re-seeking on micro-jitter between
// two HTMLVideoElement clocks.
$effect(() => {
	void store.currentTime;
	if (!cameraEl || !videoEl) return;
	if (Number.isNaN(videoEl.currentTime)) return;
	const want = trackTimeAt(videoEl.currentTime, cameraOffsetMs);
	if (Math.abs(cameraEl.currentTime - want) > 0.15) cameraEl.currentTime = want;
});

$effect(() => {
	if (!cameraEl) return;
	if (store.isPlaying) {
		if (videoEl) cameraEl.currentTime = trackTimeAt(videoEl.currentTime, cameraOffsetMs);
		void cameraEl.play().catch(() => {
			/* rejects without a gesture; the transport will retry */
		});
	} else {
		cameraEl.pause();
	}
});

//  Lifecycle & reactive wiring
onMount(() => {
	void initEngine();
	document.addEventListener("visibilitychange", onVisibilityChange);
	watchDpr();
	const ro = new ResizeObserver(() => {
		// Read layout here (rarely) instead of in the 60fps draw loop.
		if (containerEl) {
			containerW = containerEl.clientWidth;
			containerH = containerEl.clientHeight;
		}
		requestRedraw();
	});
	if (containerEl) ro.observe(containerEl);
	return () => ro.disconnect();
});

onDestroy(() => {
	document.removeEventListener("visibilitychange", onVisibilityChange);
	dprQuery?.removeEventListener("change", onDprChange);
	stopVideoFrameLoop();
	if (rafHandle !== null) cancelAnimationFrame(rafHandle);
	clearTimeout(mbRecoverTimer);
	engineDriver?.dispose();
	engineDriver = null;
	smoother?.dispose();
	smoother = null;
	mbSource?.dispose();
	mbSource = null;
});

function scheduleMbRecover() {
	clearTimeout(mbRecoverTimer);
	mbHealthyFrames = 0;
	mbRecoverTimer = setTimeout(runMbRecover, MB_RECOVER_DELAY_MS);
}

/** Re-create the MediaBunny source after a transient decode failure. */
function runMbRecover() {
	mbRecoverTimer = undefined;
	if (!video && !videoSrc) return;
	loadedMbSrc = "";
	mbRecoverNonce++;
}

// MediaBunny frame source (re)created when the media src changes. Owns its own
// worker + decoder; disposed and rebuilt per source. A decode failure (e.g.
// an unsupported codec — see `unsupported-formats.ts` in @recast/media) leaves
// mbSource null so draw() falls back to the <video> element automatically.
$effect(() => {
	// Prefer the ref: a `blob:` URL through UrlSource can degrade to a
	// whole-file fetch, while a File ref slices lazily off disk.
	const src: MediaRef | null = video ?? (videoSrc ? { kind: "url", url: videoSrc } : null);
	// Read so a recovery bump re-runs this effect; the rebuild also resets
	// loadedMbSrc, so the same-src guard below doesn't short-circuit it.
	void mbRecoverNonce;
	// No src: tear down any live engine and fall back to the <video> path.
	if (!src) {
		clearTimeout(mbRecoverTimer);
		mbRecoverTimer = undefined;
		mbRecoverAttempts = 0;
		if (mbSource) {
			mbSource.dispose();
			mbSource = null;
		}
		mbReady = false;
		webcodecsActive = false;
		loadedMbSrc = "";
		picClock.pause();
		requestRedraw();
		return;
	}
	const key = mediaRefKey(src);
	if (key === loadedMbSrc) return;
	loadedMbSrc = key;
	mbReady = false;
	mbFallbackNotified = false;
	loggedFallbackFrameError = false;
	webcodecsActive = false;
	hasRenderedFrame = false;
	lastPublishedTime = -1;
	mbSource?.dispose();
	mbSource = null;
	let cancelled = false;
	createMediabunnySource(src, {
		durationSec: store.metadata?.duration,
		fps: store.metadata?.fps,
	})
		.then((source) => {
			if (cancelled) {
				source.dispose();
				return;
			}
			rebuildFrameRing(source.width, source.height);
			// Upload and hand back in the same tick. Holding decoded frames is
			// what starved the decoder at 4K until it stopped emitting.
			source.onFrameDecoded = (frame, tsUs) => {
				engineDriver?.putScreenFrame(frame, tsUs);
				// Frames flowing again after a recovery: clear the streak so a
				// later, unrelated failure gets its full retry budget.
				if (mbRecoverAttempts > 0 && ++mbHealthyFrames > 30) {
					mbRecoverAttempts = 0;
					mbHealthyFrames = 0;
				}
			};
			source.onFrame = () => requestRedraw();
			// A dead decode run freezes the picture. A transient GPU reset gets a
			// bounded auto-rebuild; a permanent failure (unsupported codec) hands
			// back to <video>, which is worse quality but still moves.
			source.onError = (err) => {
				if (mbSource !== source) return;
				const recover = shouldRecoverMbSource(err.code, mbRecoverAttempts);
				console.error(
					`MediaBunny decode failed mid-playback; ${recover ? "rebuilding" : "falling back"}`,
					err,
				);
				analytics.capture("mediabunny_preview_fallback", { reason: err.code });
				mbReady = false;
				webcodecsActive = false;
				mbSource = null;
				source.dispose();
				requestRedraw();
				if (recover) {
					mbRecoverAttempts++;
					scheduleMbRecover();
				} else {
					// Permanent fallback (not a recoverable GPU reset): tell the user.
					notifyPreviewFallback(err.code);
				}
			};
			// Telemetry: the engine initialised successfully.
			const tier = resolutionTier(source.width, source.height);
			analytics.capture("mediabunny_preview_init", {
				width: source.width,
				height: source.height,
				fps: Math.round(source.fps),
				resolution: tier,
				ingestion: source.ingestion,
			});
			// One aggregate throughput sample, emitted when this source is disposed.
			source.onStats = (s) => {
				analytics.capture("mediabunny_preview_perf", {
					avg_fps: Math.round(s.avgFps),
					min_fps: Math.round(s.minFps),
					max_late_ms: Math.round(s.maxLateMs),
					max_av_drift_ms: Math.round(maxAvDriftSec * 1000),
					width: source.width,
					height: source.height,
					fps: Math.round(source.fps),
					resolution: tier,
				});
			};
			mbSource = source;
			mbReady = true;
			webcodecsActive = true;
			// Seed the picture clock to the current transport so flipping onto
			// the MediaBunny path (which may happen mid-playback, once demux
			// finishes) doesn't jump.
			picClock.setDuration(originalToOutput(store.timeMap, store.outPoint));
			picClock.seek(originalToOutput(store.timeMap, videoEl?.currentTime ?? 0));
			if (store.isPlaying) picClock.play();
			requestRedraw();
		})
		.catch((err) => {
			console.warn("MediaBunny source unavailable; using <video> fallback:", err);
			// Telemetry: how often real users silently drop to <video>.
			const reason = classifyMbError(err);
			analytics.capture("mediabunny_preview_fallback", { reason });
			notifyPreviewFallback(reason);
		});
	return () => {
		cancelled = true;
	};
});

// Cursor track (re)load when path changes
$effect(() => {
	void cursorPath;
	void loadCursorTrackIfNeeded();
});

// The worker decodes the picture, so a playing <video> would decode the same
// file a second time and compete for the decoder's output surfaces. Keep it
// paused as a seek-only transport; it stays mounted for the fallback path.
$effect(() => {
	void store.isPlaying;
	if (!videoEl) return;
	if (mbReady) {
		if (!videoEl.paused) videoEl.pause();
	} else if (store.isPlaying && videoEl.paused) {
		// Falling back mid-playback (a dead decode run) leaves the element
		// paused, since the page only plays it on the transport transition.
		void videoEl.play().catch(() => {
			/* rejects without a gesture; the transport will retry */
		});
	}
});

// Background (re)load when type/value changes, or when an asset:<id>
// download lands and the cached path becomes available.
$effect(() => {
	void store.backgroundType;
	void store.backgroundValue;
	if (store.backgroundValue.startsWith("asset:") && !store.backgroundValue.startsWith("asset://")) {
		const id = store.backgroundValue.slice("asset:".length);
		void assetsStore.paths[id];
		void assetsStore.thumbPaths[id];
	}
	void loadBackgroundIfNeeded();
	requestRedraw();
});

// Redraw on any visual property change
$effect(() => {
	// Track every dependency that affects the rendered frame
	void store.padding;
	void store.backgroundBlur;
	void store.borderRadius;
	void store.currentTime;
	void store.metadata;
	void store.cursorSettings;
	void store.zoomRegions;
	void store.shadow;
	void store.segmentAnims;
	requestRedraw();
});

// Start/stop the per-video-frame draw loop with playback. In the WebCodecs
// path, also run the picture clock so output time advances while playing.
$effect(() => {
	// A browser export shares this GPU + decoder. Suspend continuous playback while
	// it renders (that 60fps decode loop is what starved the export's context), but
	// leave the frame up and paused scrubs live — it stays watchable. isPlaying is
	// untouched, so playback auto-resumes when the render finishes.
	const suspendForExport = exportActivity.renderingInBrowser;
	if (store.isPlaying && !suspendForExport) {
		// Seed + start the picture clock ONLY on the paused→playing transition.
		// This effect ALSO re-runs whenever effectiveCuts/outPoint change; the
		// `!picClock.playing` guard stops those re-runs from re-seeding the clock
		// to the (lagging) <video> time mid-playback, which jumped it BACKWARD
		// and forced the decoder into a reset-thrash (the ~8 fps bug).
		if (!picClock.playing) {
			// Capture the end state before setDuration re-clamps the time.
			const wasAtEnd = picClock.atEnd;
			// Duration = output (post-cut) length of the kept region, so the
			// clock clamps at the true end of the edited timeline.
			picClock.setDuration(originalToOutput(store.timeMap, store.outPoint));
			// Restart from the top if we'd just finished; otherwise resume from
			// the current transport position. (Seeding blindly from the <video>
			// time parked the clock at the end on replay → stuck frame.)
			picClock.seek(wasAtEnd ? 0 : originalToOutput(store.timeMap, videoEl?.currentTime ?? 0));
			picClock.play();
			endHandled = false;
		}
		startVideoFrameLoop();
	} else {
		picClock.pause();
		stopVideoFrameLoop();
		requestRedraw();
	}
});

// Hook video element events
function handleSeeked() {
	// While PAUSED, a scrub/frame-step moved the transport, so realign the
	// picture clock to it. During play the clock is the master, so ignore the
	// `seeked` events our own drift-correction triggers.
	if (!store.isPlaying && videoEl) {
		picClock.seek(originalToOutput(store.timeMap, videoEl.currentTime));
	}
	requestRedraw();
	onSeeked?.();
}
function handleLoadedData() {
	isReady = true;
	requestRedraw();
	onReady();
}

// True when the user is actively editing annotations AND the global hide
// is off. The scrim/ring/canvas-tint key off this single derived so the
// visual model lives in one place.
const isAnnotationActive = $derived(
	store.activePanel === "annotations" && !store.annotationsGloballyHidden,
);
</script>

<div
	bind:this={containerEl}
	class="relative flex h-full w-full max-w-280 items-center justify-center overflow-hidden transition-all duration-200 ease-out motion-reduce:transition-none"
>
	<div
		bind:this={previewRectEl}
		data-annotations-active={isAnnotationActive}
		class="group/preview relative inline-block rounded-[inherit] outline-2 outline-offset-2 outline-transparent transition-[box-shadow,outline-color] duration-200 ease-out motion-reduce:transition-none data-[annotations-active=true]:outline-primary/30 data-[annotations-active=true]:shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-primary)_25%,transparent)]"
	>
		<canvas
			bind:this={canvasEl}
			class="block max-h-full max-w-full transition-opacity duration-200 ease-out motion-reduce:transition-none group-data-[annotations-active=true]/preview:opacity-90"
		></canvas>
		{#if paintFailed && !engineFailed}
			<!-- Without this the canvas keeps showing the last good frame, so
			     every later edit reads as "the app ignored me". -->
			<div
				class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-background/95 p-6 text-center"
				role="alert"
			>
				<div class="text-sm font-semibold text-foreground">Preview is out of date</div>
				<p class="max-w-md text-xs leading-relaxed text-muted-foreground">
					The preview couldn't redraw, so it's showing an older frame. Your edits are
					saved and export is unaffected.
				</p>
				<Button variant="outline" size="sm" onclick={() => requestRedraw()}>Try again</Button>
			</div>
		{/if}
		{#if engineFailed}
			<div
				class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 bg-background/95 p-6 text-center"
				role="alert"
			>
				<div class="text-sm font-semibold text-foreground">Preview unavailable on this device</div>
				<p class="max-w-md text-xs leading-relaxed text-muted-foreground">
					The preview needs WebGPU or WebGL2, and neither started. Updating your GPU
					driver usually fixes this. Export is unaffected.
				</p>
				<p class="max-w-md text-xs leading-relaxed text-muted-foreground">
					{engineFailed}
				</p>
			</div>
		{/if}

		<!-- Annotation scrim: primary-tinted darkening between the composite and
			 the overlay so shapes pop. Opacity 0 on every other tab. -->
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 bg-foreground/12 mix-blend-multiply opacity-0 transition-opacity duration-200 ease-out motion-reduce:transition-none group-data-[annotations-active=true]/preview:opacity-100"
		></div>
		<AnnotationOverlay
			{store}
			{videoEl}
			targetEl={previewRectEl}
			previewTime={smoothPreviewTime ?? undefined}
		/>
		<TextAnnotationLayer
			{store}
			{videoEl}
			targetEl={previewRectEl}
			previewTime={smoothPreviewTime ?? undefined}
		/>
		<div class="contents transition-opacity duration-200 ease-out motion-reduce:transition-none group-data-[annotations-active=true]/preview:opacity-55">
			<FocusOverlay {store} {videoEl} targetEl={previewRectEl} />
		</div>
		<!-- Owns its own video element, synced via store.currentTime. -->
		<CameraOverlay
			{store}
			hasCamera={!!cameraSrc}
			targetEl={previewRectEl}
			previewTime={smoothPreviewTime ?? 0}
		/>
		<CaptionOverlay {store} previewTime={smoothPreviewTime ?? undefined} />
	</div>

	{#if videoSrc}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			src={videoSrc}
			crossorigin="anonymous"
			ontimeupdate={onTimeUpdate}
			onended={onEnded}
			onloadedmetadata={onLoadedMetadata}
			onloadeddata={handleLoadedData}
			oncanplay={handleLoadedData}
			onseeked={handleSeeked}
			onerror={onError}
			class="pointer-events-none absolute h-px w-px opacity-0"
			style="visibility: hidden;"
			playsinline
			preload="metadata"
			muted
		></video>
		<!-- Legacy scout decoder: only read on the `!mbReady` path, so mounting it
		     while the worker is live would be a third decode pipeline for nothing. -->
		{#if !mbReady && store.effectiveCuts.length > 0}
			<!-- svelte-ignore a11y_media_has_caption -->
			<video
				bind:this={scoutEl}
				src={videoSrc}
				crossorigin="anonymous"
				class="pointer-events-none absolute h-px w-px opacity-0"
				style="visibility: hidden;"
				playsinline
				preload="metadata"
				muted
			></video>
		{/if}
	{/if}
	{#if cameraSrc && store.cameraOverlay.enabled}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={cameraEl}
			src={cameraSrc}
			class="pointer-events-none absolute h-px w-px opacity-0"
			style="visibility: hidden;"
			playsinline
			preload="auto"
			muted
		></video>
	{/if}

	{#if !isReady}
		<div class="pointer-events-none absolute inset-0 flex items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" />
			<span>Loading preview</span>
		</div>
	{/if}
</div>
