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
import { cameraPlaybackRate, trackTimeAt } from "../lib/editor/track-offsets";
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
	cameraStall,
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
	// biome-ignore lint/suspicious/noConfusingVoidType: a handler may return true or nothing at all.
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
// Per-frame picture time for smooth DOM overlays; store.currentTime is throttled to ~25Hz. Null until draw() has run once.
let smoothPreviewTime = $state<number | null>(null);
let isReady = $state(false);
/** The camera feed. The compositor draws the bubble, so the preview owns the
 *  element and hands it frames; the overlay is only its hit target. */
let cameraEl = $state<HTMLVideoElement | null>(null);
// Pre-decodes the first post-cut frame to mask the primary element's seek latency; seeked, never played.
let scoutEl = $state<HTMLVideoElement | null>(null);

let lastBgKey = "";
/// The Rust/wgpu compositor, the only renderer the preview has.
let engineDriver = $state<PreviewEngineDriver | null>(null);
let engineFailed = $state<string | null>(null);
// One line per session, so a wrong backend or an empty composite shows without verbose logging.
let loggedEngineFrame = false;
let loggedFallbackFrameError = false;
// Rolling composite times, reported once per window while playing: does WebGPU hold the frame budget here?
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
// Worker-side MediaBunny source: the composite samples a frame WE decode, so a cut never waits on a native seek; null falls back to <video>.
let mbSource: MediabunnyVideoSource | null = null;
let mbReady = $state(false);
let loadedMbSrc = "";
// One notice per source when the hardware preview drops to <video>, or the failure is a silent blank screen.
let mbFallbackNotified = false;
function notifyPreviewFallback(reason: string) {
	if (mbFallbackNotified) return;
	mbFallbackNotified = true;
	toast.warning("Using the standard preview player", {
		description: `Hardware preview couldn't start for this video (${reason}). Playback works; scrubbing may be slower.`,
	});
}
// A GPU-process reset under scrub-thrash kills the decoder but is recoverable; without this the session stayed on <video>.
const MB_RECOVER_DELAY_MS = 400;
let mbRecoverAttempts = 0;
let mbHealthyFrames = 0;
let mbRecoverTimer: ReturnType<typeof setTimeout> | undefined;
let mbRecoverNonce = $state(0);
// True once the engine has presented a frame; an early return from draw() would otherwise clear to black.
let hasRenderedFrame = false;
/** Worst |video − audio| seen this session; reported with the perf sample. */
let maxAvDriftSec = 0;
const audioStall = new AudioStallMonitor();
let audioStalledReported = false;
// Throttled: the write fans out to overlays, timeline and waveform, and every-rAF writes starve frame delivery.
let lastPublishedTime = -1;
// Fires the end-of-timeline stop once per play session, not every frame while the clock sits clamped.
let endHandled = false;

// A free-running OUTPUT clock drives the picture: a <video> currentTime stalls during its own seek and freezes every cut.
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

// Recomputing only when this signature changes keeps playback cheap on long recordings.
let smoothingSignature = "";
// The engine keys its upload on this, not the smoothing signature, which changes on request rather than result.
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
	// The resolved cache path is in the key, so the texture reloads when a late `asset:<id>` download lands.
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
			// Not cached yet (first-run offline or still downloading); flat background until a later tick re-runs this.
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
		// Press events come from raw samples, keyed by timestamps, which smoothing never moves.
		pressEvents = buildPressEvents(cursorSamplesRaw);
		if (!smoother) {
			smoother = new CursorSmoother((samples) => {
				cursorVersion++;
				cursorSamples = samples;
				requestRedraw();
			});
		}
		// By URL: the worker re-reads the track rather than us cloning ~225k sample objects on the main thread.
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

// Called once per draw(): the signature is set immediately as an in-flight marker so the per-frame call can't re-fire it.
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

// --- Per-frame memoization: the draw loop runs at 60fps, so these caches stop identical re-parses every frame.
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

// Cached by the ResizeObserver: reading clientWidth in the draw loop forces a synchronous reflow every frame.
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

	// devicePixelRatio for crispness, capped at the composition's native resolution and at 2160p.
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

// --- Render. Each cut-skip seek is issued ONCE: re-assigning currentTime mid-seek thrashes the decoder into a stall.
let cutSkipTarget: number | null = null;
// Where the scout is seeking, so the same pre-decode seek is not re-issued every frame.
let scoutSeekTarget: number | null = null;

// Scout pre-decode lead and primary jump lead (s); the wider scout window has the post-cut frame ready at the boundary.
const SCOUT_PRESEEK_LOOKAHEAD = 0.6;
const CUT_JUMP_LOOKAHEAD = 0.12;
// OUTPUT-seconds lead for warming the post-cut GOP on the worker's scout decoder; covers ~1s GOPs fully.
const WC_PREFETCH_LOOKAHEAD = 2.0;
// A seek can land a frame or two off target, so this is how close the scout must be to stand in.
const SCOUT_READY_EPS = 0.1;

/** True when the scout has the post-cut frame decoded and ready to sample. */
function scoutReadyAt(t: number): boolean {
	return (
		scoutEl !== null &&
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

	// Signature guard makes this one string compare when nothing changed since the last frame.
	ensureSmoothingCurrent();

	// WebCodecs: the gapless OUTPUT clock is master, so <video> seek stalls can't freeze the picture. Legacy: <video> is the clock.
	const usingPicClock = mbReady;
	let playbackTime: number;
	if (usingPicClock && store.isPlaying) {
		// External scrub while playing: re-seat the clock on a currentTime we didn't publish, or the seek snaps back.
		if (Math.abs(store.currentTime - lastPublishedTime) > 0.05) {
			picClock.seek(originalToOutput(store.timeMap, store.currentTime));
			lastPublishedTime = store.currentTime;
			endHandled = false;
		}
		// Audio runs on the sound card's clock and the picture on wall time; pull back once the gap is perceptible.
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
		// Ask the host before stopping: a false-then-true isPlaying flip in one tick batches to no change and the picture sticks.
		if (picClock.atEnd && !endHandled) {
			if (onEnded?.() === true) {
				// The host moved the transport; follow it and keep playing.
				picClock.seek(originalToOutput(store.timeMap, store.currentTime));
				lastPublishedTime = store.currentTime;
			} else {
				// The clock clamps at its duration, so without this the picture freezes on the last frame while still 'playing'.
				endHandled = true;
				store.isPlaying = false;
			}
		}
		// ~25 Hz, not every rAF: the fan-out starved frame delivery. Always publish on a backward step or a jump.
		if (playbackTime >= lastPublishedTime + 0.04 || playbackTime < lastPublishedTime) {
			store.currentTime = playbackTime;
			lastPublishedTime = playbackTime;
		}
		// Keeps the paused <video> roughly aligned so the legacy fallback can take over mid-playback.
		if (videoEl && !videoEl.seeking && Math.abs(videoEl.currentTime - playbackTime) > 0.25) {
			videoEl.currentTime = playbackTime;
		}
	} else if (usingPicClock) {
		// Paused on the MediaBunny path, so the store owns time; the element must not decode alongside the worker.
		playbackTime = store.currentTime;
	} else {
		// Legacy path: the <video> owns time, and handleSeeked realigns the picture clock so resume continues from here.
		playbackTime = videoEl ? videoEl.currentTime : store.currentTime;
		// From rAF, not `timeupdate`: that ~250ms tick made the scrubber, playhead and overlays advance in visible 4Hz steps.
		if (
			store.isPlaying &&
			(playbackTime >= lastPublishedTime + 0.04 || playbackTime < lastPublishedTime)
		) {
			store.currentTime = playbackTime;
			lastPublishedTime = playbackTime;
		}
	}

	// Unthrottled, unlike the store fan-out above: one number write, read only by the camera bubble.
	smoothPreviewTime = playbackTime;

	// Legacy cut skip: the scout pre-decodes cut.end while the primary jumps there, so the swap hides the seek. Issued once per cut.
	let frameEl: HTMLVideoElement | null = videoEl;
	const activeCuts = store.effectiveCuts;
	// Off for the WebCodecs path: its output clock is gapless, and decoding through a removed region would flood the decoder.
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
			// At the boundary, jump the primary and mask its seek with the scout's pre-decoded frame.
			if (playbackTime + CUT_JUMP_LOOKAHEAD >= cut.start) {
				if (cutSkipTarget !== cut.end && !videoEl.seeking) {
					cutSkipTarget = cut.end;
					videoEl.currentTime = cut.end;
				}
				if (scoutReadyAt(cut.end)) {
					// Draw the scout's frame this tick, no visible freeze.
					frameEl = scoutEl;
				} else {
					// Scout not ready (sparse keyframes): hold the last frame until the primary settles.
					return;
				}
			} else {
				// Approaching but not yet at the boundary: keep playing the primary normally.
				cutSkipTarget = null;
			}
		} else {
			// Outside any cut window: clear so the next cut can fire.
			cutSkipTarget = null;
			scoutSeekTarget = null;
		}
	}

	// Warms the post-cut GOP on the worker's scout decoder before a crossing; the worker dedupes per GOP.
	if (usingPicClock && store.isPlaying && mbSource && mbReady && activeCuts.length > 0) {
		const lookaheadOrig = outputToOriginal(store.timeMap, picClock.time + WC_PREFETCH_LOOKAHEAD);
		const upcoming = activeCuts.find((c) => c.start > playbackTime && c.start <= lookaheadOrig);
		if (upcoming) mbSource.prefetch(upcoming.end);
	}

	{
		const engineStartedAt = performance.now();
		// The engine evaluates the scene itself, so it takes OUTPUT time; `playbackTime` only picks a decoded frame.
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
			// `<video>` fallback: no decode stream to ring, so the frame uploads and binds in the same tick.
			const tUs = Math.max(0, Math.round(playbackTime * 1e6));
			let frame: VideoFrame | null = null;
			try {
				frame = new VideoFrame(frameEl, { timestamp: tUs });
				engineDriver.putScreenFrame(frame, tUs);
				bound = engineDriver.bindScreenFrame(tUs, 0);
			} catch (err) {
				// readyState 2 arrives before a decodable frame, so this fires on every source change; once is enough.
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
		reportCameraStall();
		if (!isReady) isReady = true;
		const finishedAt = performance.now();
		engineFrameMs.push(finishedAt - engineStartedAt);
		reportEngineFrameTimes(finishedAt);
	}
}

function requestRedraw() {
	// Without this guard the ~25Hz publish inside draw() re-entered and scheduled a second composite (~85 draws/sec).
	if (wcRafHandle !== null) return;
	if (rafHandle !== null) return;
	rafHandle = requestAnimationFrame(() => {
		rafHandle = null;
		try {
			draw();
			if (paintFailed) paintFailed = false;
		} catch (err) {
			// Paused, a throw leaves the last good frame up and every later edit appears to do nothing.
			console.error("preview draw() failed:", err);
			paintFailed = true;
		}
	});
}

// --- Playback frame loop (rAF); see startVideoFrameLoop.
let wcRafHandle: number | null = null;
// Consecutive draw() failures; a bad frame must not kill the loop.
let drawErrors = 0;
// The composite is stuck on a stale frame; surfaced in the template.
let paintFailed = $state(false);

function startVideoFrameLoop() {
	// rAF, not requestVideoFrameCallback: rVFC stalls during the seek we issue at a cut, exactly when we must keep painting.
	if (wcRafHandle !== null) return;
	const loop = () => {
		try {
			draw();
			drawErrors = 0;
			if (paintFailed) paintFailed = false;
		} catch (err) {
			// A dead loop freezes the preview and reads as a crash: log once, tolerate transients, stop if persistent.
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
		// `requestRedraw` swallowed property changes during playback; paint once so the paused frame is current.
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

// devicePixelRatio has no change event and ResizeObserver ignores scale-only changes, so a DPI move kept the old buffer.
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

// Reads the whole render state, so it re-runs on any store write; the driver drops an unchanged scene.
$effect(() => {
	if (!engineDriver) return;
	const state = store.toRenderState();
	const meta = store.metadata;
	const timeMap = store.timeMap;
	untrack(() => {
		if (!engineDriver) return;
		if (meta?.width && meta?.height) engineDriver.setSourceSize(meta.width, meta.height);
		// Before the scene: the axis defines what output time MEANS, or a dropped cut resolves on the old axis.
		engineDriver.setTimeMap(timeMap);
		engineDriver.syncScene(state);
		requestRedraw();
	});
});

/** Version of the cursor track last handed to the engine. Stringifying a
 *  225-second track every frame would cost more than the composite. */
let engineCursorSignature = "";

// The camera silently missing is the shape of bug this codebase keeps finding, so the preview says which link broke rather than drawing nothing.
let loggedCameraStall = false;
function reportCameraStall() {
	if (loggedCameraStall || !engineDriver) return;
	const reason = cameraStall({
		enabled: store.cameraOverlay.enabled,
		hasSrc: Boolean(cameraSrc),
		elementMounted: Boolean(cameraEl),
		readyState: cameraEl?.readyState ?? 0,
		videoWidth: cameraEl?.videoWidth ?? 0,
		gated: cameraFrameGated,
		frameReady: cameraFrameReady,
		boundInEngine: engineDriver.hasCameraFrame(),
	});
	if (!reason) return;
	loggedCameraStall = true;
	console.warn(`preview: the camera is enabled but not visible - ${reason}`);
}

/** One slot, uploaded and bound in the same tick: the element is a seek-only
 *  transport, so there is no decode stream to buffer. */
function putCameraFrame(timestampUs: number) {
	if (!engineDriver || !cameraEl) return;
	if (cameraEl.readyState < 2 || cameraEl.videoWidth === 0) return;
	// Only re-upload when the element presented a new frame (rVFC). The last frame stays bound and drawn, so a paused or between-frames draw does no GPU copy. Gate off when rVFC is unavailable so the camera never goes blank.
	if (cameraFrameGated && !cameraFrameReady) return;
	let frame: VideoFrame | null = null;
	try {
		frame = new VideoFrame(cameraEl, { timestamp: timestampUs });
		engineDriver.putCameraFrame(frame, timestampUs);
		cameraFrameReady = false;
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
let cameraFrameReady = false;
let cameraFrameGated = false;

// Upload one camera frame per presented frame: rVFC fires on decode and on a completed seek, which a `currentTime` compare would miss (the shown frame lags the property).
$effect(() => {
	const el = cameraEl;
	if (!el || typeof el.requestVideoFrameCallback !== "function") {
		cameraFrameGated = false;
		return;
	}
	cameraFrameGated = true;
	// A remounted element starts un-ready: without this a stale ready from the last element uploads its first draw before rVFC fires.
	cameraFrameReady = false;
	let handle = el.requestVideoFrameCallback(function onFrame() {
		cameraFrameReady = true;
		requestRedraw();
		handle = el.requestVideoFrameCallback(onFrame);
	});
	return () => {
		cameraFrameGated = false;
		el.cancelVideoFrameCallback(handle);
	};
});

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

// A path that fails to decode never uploads, so the annotation is skipped rather than drawn as a placeholder.
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

// A style with no sprite uploads nothing, which leaves the engine drawing its dot.
$effect(() => {
	if (!engineDriver) return;
	const style = store.cursorSettings.style;
	untrack(() => {
		void loadCursorSprites(style, resolveCursorDataUrl).then((sprites) => {
			if (engineDriver?.setCursorSprites(style, sprites)) requestRedraw();
		});
	});
});

// Read the store playhead, not the hidden <video>: on the WebCodecs path it is not kept aligned, so the camera would stick at the start. Tolerance avoids re-seeking on micro-jitter.
$effect(() => {
	if (!cameraEl) return;
	const t = store.currentTime;
	if (Number.isNaN(t)) return;
	cameraEl.playbackRate = cameraPlaybackRate(store.timeMap, t);
	const want = trackTimeAt(t, cameraOffsetMs);
	if (Math.abs(cameraEl.currentTime - want) > 0.15) cameraEl.currentTime = want;
});

$effect(() => {
	if (!cameraEl) return;
	if (store.isPlaying) {
		// Seed once on play, untracked: subscribing to currentTime would re-seek the playing element ~25Hz and stall its decoder. The tolerance effect above corrects drift.
		const t = untrack(() => store.currentTime);
		cameraEl.playbackRate = cameraPlaybackRate(store.timeMap, t);
		cameraEl.currentTime = trackTimeAt(t, cameraOffsetMs);
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

// Owns its worker and decoder, rebuilt per source; a decode failure leaves mbSource null and draw() uses <video>.
$effect(() => {
	// Prefer the ref: a `blob:` URL through UrlSource can degrade to a whole-file fetch, a File ref slices lazily.
	const src: MediaRef | null = video ?? (videoSrc ? { kind: "url", url: videoSrc } : null);
	// Read so a recovery bump re-runs this; the rebuild resets loadedMbSrc so the same-src guard can't short-circuit.
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
			// Upload and hand back in the same tick: holding decoded frames starved the 4K decoder until it stopped emitting.
			source.onFrameDecoded = (frame, tsUs) => {
				engineDriver?.putScreenFrame(frame, tsUs);
				// Frames flowing again after a recovery: clear the streak so a later failure gets its full retry budget.
				if (mbRecoverAttempts > 0 && ++mbHealthyFrames > 30) {
					mbRecoverAttempts = 0;
					mbHealthyFrames = 0;
				}
			};
			source.onFrame = () => requestRedraw();
			// A transient GPU reset gets a bounded auto-rebuild; a permanent failure hands back to <video>.
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
			// Seed from the store transport, not the hidden <video>: on the WebCodecs path it is not kept aligned while paused, so its currentTime is stale and a recovery rebuild would jump to 0.
			picClock.setDuration(originalToOutput(store.timeMap, store.outPoint));
			picClock.seek(originalToOutput(store.timeMap, store.currentTime));
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

// A playing <video> would decode the same file twice and fight for output surfaces; keep it a seek-only transport.
$effect(() => {
	void store.isPlaying;
	if (!videoEl) return;
	if (mbReady) {
		if (!videoEl.paused) videoEl.pause();
	} else if (store.isPlaying && videoEl.paused) {
		// Falling back mid-playback leaves the element paused, since the page only plays it on the transport transition.
		void videoEl.play().catch(() => {
			/* rejects without a gesture; the transport will retry */
		});
	}
});

// Reloads on type or value change, and when an `asset:<id>` download makes the cached path available.
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

// Starts and stops the draw loop with playback, and runs the picture clock on the WebCodecs path.
$effect(() => {
	// A browser export shares this GPU and decoder; suspend playback but leave the frame up and scrubbable.
	const suspendForExport = exportActivity.renderingInBrowser;
	if (store.isPlaying && !suspendForExport) {
		// Seed only on the paused-to-playing transition: cut/outPoint re-runs jumped the clock BACKWARD into decoder thrash.
		if (!picClock.playing) {
			// Capture the end state before setDuration re-clamps the time.
			const wasAtEnd = picClock.atEnd;
			// Duration is the output length of the kept region, so the clock clamps at the true end of the edit.
			picClock.setDuration(originalToOutput(store.timeMap, store.outPoint));
			// Restart from the top if we just finished; seeding blindly from <video> parked the clock at the end on replay.
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
	// Paused, a scrub moved the transport, so realign; during play the clock is master and its own seeks are ignored.
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

// Scrim, ring and canvas tint all key off this one derived, so the visual model lives in one place.
const isAnnotationActive = $derived(
	store.activePanel === "annotations" && !store.annotationsGloballyHidden,
);
</script>

<div
	bind:this={containerEl}
	class="relative flex h-full w-full max-w-280 items-center justify-center overflow-hidden bg-[var(--editor-canvas)] transition-all duration-200 ease-out motion-reduce:transition-none"
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
			crossorigin="anonymous"
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
