<script lang="ts">
	import { analytics } from "$lib/analytics/client";
	import { resolveAsset } from "$lib/assets";
	import { computeCanvasGeometry } from "$lib/canvas-geometry";
	import { CursorSmoother } from "$lib/cursor/smoother";
	import { smoothingStrengthToSigmaMs } from "$lib/cursor/smoothing";
	import { CAMERA_OVERLAY_UI_ENABLED } from "$lib/feature-flags";
	import { PlaybackClock } from "$lib/playback/clock";
	import type { MediabunnyVideoSource } from "@recast/media/playback";
	import { createMediabunnySource } from "$lib/playback/mediabunny";
	import {
	  cursorSpriteHotspot,
	  resolveBackgroundWireValue,
	  resolveCursorDataUrl,
	  resolveCursorSprite,
	} from "$lib/registry";
	import { RenderCore } from "./render-core";
	import { WebGL2Backend } from "./webgl2-backend";
	import { assetsStore } from "$lib/stores/assets-store.svelte";
	import { type EditorStore } from "$lib/stores/editor-store.svelte";
	import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
	import { Spinner } from "@recast/ui/spinner";
	import { toast } from "@recast/ui/sonner";
	import { convertFileSrc } from "@tauri-apps/api/core";
	import { onDestroy, onMount } from "svelte";
	import AnnotationOverlay from "./_components/AnnotationOverlay.svelte";
	import AnnotationStatusRail from "./_components/AnnotationStatusRail.svelte";
	import CameraOverlay from "./_components/CameraOverlay.svelte";
	import CaptionOverlay from "./_components/CaptionOverlay.svelte";
	import FocusOverlay from "./_components/FocusOverlay.svelte";
	import TextAnnotationLayer from "./_components/TextAnnotationLayer.svelte";
	import {
	  buildPressEvents,
	  type PressEvent,
	} from "./cursor-animation.logic";
	import { buildGradientUniforms } from "./gradient.logic";
	import {
	  classifyMbError,
	  resolutionTier,
	  shouldRecoverMbSource,
	  type CursorSampleJS,
	  type IdlePeriodJS,
	} from "./video-preview.logic";
	import { AudioStallMonitor, resolveAvSync } from "$lib/playback/av-sync";
	import { FrameTextureRing } from "$lib/playback/frame-textures";
	import { textureRingFrames } from "@recast/media";

	interface Props {
		store: EditorStore;
		videoEl: HTMLVideoElement | null;
		videoSrc: string;
		cursorPath: string | null;
		/** convertFileSrc(camera.mp4) for this project, or empty when no
		 *  camera was recorded. Forwarded to CameraOverlay; the overlay
		 *  renders nothing when this is empty. */
		cameraSrc?: string;
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
		cursorPath,
		cameraSrc = "",
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
	// WebView doesn't expose WebGL2, so surface an actionable message rather than a
	// silently blank canvas (old integrated GPUs, broken/outdated drivers).
	let webgl2Unsupported = $state(false);
	/** GPU context lost (driver reset / TDR); recoverable, unlike the above. */
	let glLost = $state(false);
	let containerEl: HTMLDivElement | null = $state(null);
	/** Shrink-wrap around the WebGL canvas so the annotation overlay can sit
	 * on top of it at the same rendered rect regardless of letterboxing. */
	let previewRectEl: HTMLDivElement | null = $state(null);
	// Per-FRAME picture time for smooth DOM overlays (camera bubble). store.currentTime
	// is throttled to ~25Hz to spare the timeline/waveform fan-out; the camera grow
	// tracks the zoom curve, so it reads this instead to stay as smooth as the shader.
	let smoothPreviewTime = $state(0);
	let isReady = $state(false);
	// Internal decoder that pre-decodes the first post-cut frame to mask the
	// primary element's seek latency. Only seeked once per cut, never played.
	let scoutEl = $state<HTMLVideoElement | null>(null);

	let gl: WebGL2RenderingContext | null = null;
	let backend: WebGL2Backend | null = null;
	let renderCore: RenderCore | null = null;
	let videoTex: WebGLTexture | null = null;
	let bgTex: WebGLTexture | null = null;
	let bgTexReady = false;
	let lastBgKey = "";

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
	// Decoded frames live here as textures we own, so each VideoFrame goes back
	// to the decoder's pool immediately after upload.
	let frameRing: FrameTextureRing | null = null;
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
	let mbRecoverPending = false;
	let mbRecoverTimer: ReturnType<typeof setTimeout> | undefined;
	let mbRecoverNonce = $state(0);
	// True once a frame is in videoTex. preserveDrawingBuffer:false means an early
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

	// SVG cursor overlay state, updated each draw() for non-`dot` styles and
	// consumed by the absolutely-positioned <img>. Not $derived: the data is
	// pulled from the draw loop where the cursor sample is already evaluated.
	let svgCursor = $state<{
		visible: boolean;
		alpha: number;
		styleId: import("$lib/stores/editor-store.svelte").StoredCursorId;
		pressed: boolean;
		right: boolean; // active press was a right-click (sprite slot)
		dragging: boolean; // active press is a drag (sprite slot)
		scale: number; // JS-driven press impact curve; see pressStateAt
		canvasX: number; // source-pixel space, includes padding offset
		canvasY: number;
		compW: number;
		compH: number;
		spritePx: number; // sprite size in source pixels; render width = (spritePx/compW)*100%
	}>({
		visible: false,
		alpha: 0,
		styleId: "dot",
		pressed: false,
		right: false,
		dragging: false,
		scale: 1,
		canvasX: 0,
		canvasY: 0,
		compW: 1,
		compH: 1,
		spritePx: 32,
	});
	// Signature of the inputs that drive smoothing. Recomputing only when this
	// changes keeps playback cheap even on long recordings.
	let smoothingSignature = "";


	let pressEvents: PressEvent[] = [];

	/**
	 * (Re)build the texture ring for the live source. Sized from the source, so
	 * it must be rebuilt after a context restore too — the old handles belong to
	 * the dead context and binding them fails silently.
	 */
	function rebuildFrameRing(width: number, height: number) {
		frameRing?.dispose();
		frameRing = gl ? new FrameTextureRing(gl, textureRingFrames(width, height)) : null;
	}

	function initGL() {
		if (!canvasEl) return;
		const g = canvasEl.getContext("webgl2", {
			alpha: false,
			antialias: false,
			premultipliedAlpha: false,
			preserveDrawingBuffer: false,
		});
		if (!g) {
			console.error("WebGL2 not supported in this WebView");
			webgl2Unsupported = true;
			return;
		}
		gl = g;

		backend = WebGL2Backend.create(g);
		renderCore = new RenderCore(backend);

		// Allocate textures
		videoTex = g.createTexture();
		g.bindTexture(g.TEXTURE_2D, videoTex);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);

		bgTex = g.createTexture();
		g.bindTexture(g.TEXTURE_2D, bgTex);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);
		// Placeholder 1×1 transparent texture so the sampler is always valid
		g.texImage2D(g.TEXTURE_2D, 0, g.RGBA, 1, 1, 0, g.RGBA, g.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 0]));
	}

	//  Background loading 
	async function resolveBackgroundSrc(value: string): Promise<string> {
		if (!value) return "";
		// Extension wallpaper: `ext:<extId>:<localId>` → the pack's hydrated
		// absolute path. Unresolved (pack removed) → fallback colour, no texture.
		if (value.startsWith("ext:")) {
			const wire = resolveBackgroundWireValue(value);
			if (!wire || wire.startsWith("#")) return "";
			return convertFileSrc(wire);
		}
		// Defensive: keep gradient/colour values away from convertFileSrc, since a
		// stray write leaving a CSS gradient here while type briefly reads "image"
		// would otherwise log a bogus "File does not exist" via the asset protocol.
		if (value.includes("gradient(") || value.startsWith("#")) return "";
		if (value.startsWith("asset:") && !value.startsWith("asset://")) {
			const id = value.slice("asset:".length);
			const cached = await resolveAsset(id);
			if (cached) return convertFileSrc(cached);
			const thumb = assetsStore.thumbPaths[id];
			if (thumb) return convertFileSrc(thumb);
			return "";
		}
		if (
			value.startsWith("data:") ||
			value.startsWith("http://") ||
			value.startsWith("https://") ||
			value.startsWith("asset://") ||
			value.startsWith("/")
		) {
			// Already a URL (served or data) or a root-relative path served
			// from the frontend's static/ dir.
			return value;
		}
		// Raw filesystem path: convert to the Tauri asset protocol.
		return convertFileSrc(value);
	}

	async function loadBackgroundIfNeeded() {
		if (!gl || !bgTex) return;
		const type = store.backgroundType;
		const value = store.backgroundValue;
		// Including the resolved cache path in the key ensures the texture
		// re-loads when an `asset:<id>` download lands after an initial miss,
		// or when the thumbnail lands before the full-res does.
		let resolvedForKey = value;
		if (value.startsWith("asset:") && !value.startsWith("asset://")) {
			const id = value.slice("asset:".length);
			resolvedForKey =
				assetsStore.paths[id] ?? assetsStore.thumbPaths[id] ?? value;
		}
		const key = `${type}|${resolvedForKey}`;
		if (key === lastBgKey) return;
		lastBgKey = key;

		if (type !== "wallpaper" && type !== "image") {
			bgTexReady = false;
			return;
		}

		if (!value) {
			bgTexReady = false;
			return;
		}

		try {
			const resolvedSrc = await resolveBackgroundSrc(value);
			if (!resolvedSrc) {
				// Asset not yet cached (first-run offline, or still downloading).
				// Fall through to flat-background rendering until a later tick
				// re-runs this effect once the cache populates.
				bgTexReady = false;
				return;
			}
			const img = new Image();
			img.crossOrigin = "anonymous";
			img.src = resolvedSrc;
			await img.decode();
			if (lastBgKey !== key) return; // Superseded by another load
			gl.bindTexture(gl.TEXTURE_2D, bgTex);
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
			bgTexReady = true;
			requestRedraw();
		} catch (err) {
			console.warn("Background image load failed:", err, "value:", value);
			bgTexReady = false;
		}
	}

	//  Cursor track loading 
	async function loadCursorTrackIfNeeded() {
		if (!cursorPath || cursorPath === loadedCursorPath) return;
		try {
			const url = convertFileSrc(cursorPath);
			const res = await fetch(url);
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const json = (await res.json()) as {
				samples?: CursorSampleJS[];
				idlePeriods?: IdlePeriodJS[];
			};
			cursorSamplesRaw = json.samples ?? [];
			cursorSamples = cursorSamplesRaw;
			idlePeriods = json.idlePeriods ?? [];
			loadedCursorPath = cursorPath;
			smoothingSignature = "";
			// Publish raw samples for the Cursor panel's trajectory minimap.
			store.cursorSamplesRaw = cursorSamplesRaw;
			// Press events come from raw samples, smoothing-independent.
			// Rebuild once per track load; the result is keyed by sample
			// timestamps, which never move regardless of smoothing settings.
			pressEvents = buildPressEvents(cursorSamplesRaw);
			if (!smoother) {
				smoother = new CursorSmoother((samples) => {
					cursorSamples = samples;
					requestRedraw();
				});
			}
			smoother.load(cursorSamplesRaw);
			ensureSmoothingCurrent();
		} catch (err) {
			console.warn("Cursor track load failed:", err);
			cursorSamplesRaw = [];
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

	let gradCache: ReturnType<typeof buildGradientUniforms> | null = null;
	let gradSig = "\0";
	function currentGradient(value: string) {
		if (value !== gradSig) {
			gradCache = buildGradientUniforms(value);
			gradSig = value;
		}
		return gradCache!;
	}


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
	let loggedTexError = false;
	// Uploads a decoded video frame into the sampling texture. `el` defaults to
	// the primary playback element but may be the scout during a cut-skip mask.
	function uploadVideoFrame(el: HTMLVideoElement | null = videoEl) {
		if (!gl || !videoTex || !el) return false;
		if (el.readyState < 2 /* HAVE_CURRENT_DATA */) return false;
		if (el.videoWidth === 0 || el.videoHeight === 0) return false;
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, videoTex);
		// texImage2D from a video element is hardware-accelerated by the browser
		gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
		try {
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, el);
		} catch (err) {
			if (!loggedTexError) {
				loggedTexError = true;
				console.error(
					`WebGL texImage2D failed for video (src=${el.currentSrc || el.src}):`,
					err,
				);
			}
			return false;
		}
		return true;
	}


	// AnnotationOverlay reads this canvas back via drawImage from its OWN rAF
	// loop. With preserveDrawingBuffer:false the GL buffer is only valid for a
	// cross-canvas read within the SAME task as draw(); an out-of-task read
	// samples a cleared buffer (the blur "flicker"). Fix: mirror the composite
	// into a 2D canvas in-task after each draw() and have the overlay sample
	// that. Maintained only while a blur exists, so the common path pays nothing.
	let blurMirrorEl = $state<HTMLCanvasElement | null>(null);
	const hasBlurAnnotation = $derived(
		store.annotations.some((a) => a.kind.kind === "blur" && !a.hidden),
	);

	function syncBlurMirror() {
		if (!hasBlurAnnotation || !canvasEl) return;
		const w = canvasEl.width;
		const h = canvasEl.height;
		if (!w || !h) return;
		let mirror = blurMirrorEl ?? document.createElement("canvas");
		if (mirror.width !== w || mirror.height !== h) {
			mirror.width = w;
			mirror.height = h;
		}
		const ctx = mirror.getContext("2d");
		if (!ctx) return;
		try {
			// Same-task drawImage from a WebGL canvas captures the current
			// buffer even when preserveDrawingBuffer is false (cf. captureFrame).
			ctx.drawImage(canvasEl, 0, 0);
		} catch {
			return;
		}
		if (blurMirrorEl !== mirror) blurMirrorEl = mirror;
	}

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
		if (!gl || !renderCore || !canvasEl || !store.metadata) return;
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
				picClock.seek(
					originalToOutput(store.timeMap, store.currentTime),
				);
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
			if (
				playbackTime >= lastPublishedTime + 0.04 ||
				playbackTime < lastPublishedTime
			) {
				store.currentTime = playbackTime;
				lastPublishedTime = playbackTime;
			}
			// Keep the <video> transport roughly aligned so the legacy fallback can
			// take over mid-playback. It stays paused here (see the effect below),
			// so this is a cheap single-frame seek, not continuous decode.
			if (
				videoEl &&
				!videoEl.seeking &&
				Math.abs(videoEl.currentTime - playbackTime) > 0.25
			) {
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
		if (
			!mbReady &&
			videoEl &&
			store.isPlaying &&
			activeCuts.length > 0
		) {
			const cut = activeCuts.find(
				(c) =>
					playbackTime + SCOUT_PRESEEK_LOOKAHEAD >= c.start &&
					playbackTime < c.end - 0.02,
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
		if (
			usingPicClock &&
			store.isPlaying &&
			mbSource &&
			mbReady &&
			activeCuts.length > 0
		) {
			const lookaheadOrig = outputToOriginal(
				store.timeMap,
				picClock.time + WC_PREFETCH_LOOKAHEAD,
			);
			const upcoming = activeCuts.find(
				(c) => c.start > playbackTime && c.start <= lookaheadOrig,
			);
			if (upcoming) mbSource.prefetch(upcoming.end);
		}

		// Get the frame into the texture. With the WebCodecs engine we sample a
		// frame WE decoded for playbackTime (no <video> seek latency); fall back
		// to the <video> element while the source is still demuxing or if a frame
		// isn't ready yet, so the preview is never blank.
		let haveFrame = false;
		if (mbSource && mbReady) {
			// Floor = start of the current kept segment = the end of the most recent
			// cut at or before the playhead (0 if none). Frames before it belong to
			// a prior segment (inside the removed range) and must not be shown, or
			// the picture steps back into deleted content at the cut.
			let floorSec = 0;
			for (const c of activeCuts) {
				if (c.end <= playbackTime && c.end > floorSec) floorSec = c.end;
			}
			// Frames were uploaded to the ring as they arrived and released back
			// to the decoder; here we only pick which texture to sample.
			mbSource.advanceTo(Math.max(0, playbackTime));
			const tUs = Math.max(0, Math.round(playbackTime * 1e6));
			const floorUs = Math.max(0, Math.round(floorSec * 1e6));
			haveFrame = frameRing?.bind(tUs, floorUs) ?? false;
			// No fresh in-segment frame yet (briefly, right after a cut while the
			// post-cut GOP decodes): hold the last frame we actually displayed.
			if (!haveFrame && hasRenderedFrame) haveFrame = frameRing?.bindLast() ?? false;
		}
		if (!haveFrame && !uploadVideoFrame(frameEl)) return;
		hasRenderedFrame = true;
		// The preview has painted, so hide the spinner — whichever engine drew it.
		// Previously `isReady` came only from the <video>'s `canplay`, which forced
		// `preload="auto"` (buffering the whole file) just to clear the spinner.
		if (!isReady) isReady = true;

		// Background (re)load is driven by a $effect on its reactive inputs and by
		// onContextRestored — no per-frame call needed here (it allocated a Promise
		// + key string every frame only to early-return).

		const meta = store.metadata!;
		const geom = currentGeometry();
		if (!geom) return;

		// One pure scene→uniform evaluation + a single GL apply via RenderCore,
		// shared with the offline export renderer instead of a second compositor.
		const gradient =
			store.backgroundType === "gradient"
				? currentGradient(store.backgroundValue || "")
				: undefined;
		const frame = renderCore.renderFrame(
			{
				meta: { width: meta.width, height: meta.height },
				geom,
				canvasPxW: canvasEl.width,
				canvasPxH: canvasEl.height,
				playbackTime,
				segments: store.segments,
				segmentAnims: store.segmentAnims,
				backgroundType: store.backgroundType,
				backgroundValue: store.backgroundValue,
				backgroundBlur: store.backgroundBlur,
				backgroundImageReady: bgTexReady,
				gradient,
				borderRadius: store.borderRadius ?? 0,
				focusEnabled: store.focusEnabled,
				zoomRegions: store.zoomRegions,
				shadow: store.shadow,
				cursor: store.cursorSettings,
				cursorMotionEasing: store.cursorMotionEasing,
				cursorSamples,
				idlePeriods,
				pressEvents,
			},
			{ backgroundTex: bgTex },
		);

		// SVG cursor overlay: written only for a non-dot style, else cleared once so
		// the HTML <img> hides (the shader draws the dot cursor itself).
		if (frame.svgCursor) {
			svgCursor = frame.svgCursor;
		} else if (svgCursor.visible) {
			svgCursor = { ...svgCursor, visible: false };
		}

		// In-task mirror for blur read-back (see comment on blurMirrorEl).
		syncBlurMirror();
	}

	function requestRedraw() {
		if (rafHandle !== null) return;
		rafHandle = requestAnimationFrame(() => {
			rafHandle = null;
			try {
				draw();
			} catch (err) {
				console.error("preview draw() failed:", err);
			}
		});
	}

	//  Playback frame loop (rAF)
	// rAF handle for the preview playback loop (see startVideoFrameLoop).
	let wcRafHandle: number | null = null;
	// Consecutive draw() failures; a bad frame must not kill the loop.
	let drawErrors = 0;

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
			} catch (err) {
				// A bad frame must not kill the loop (a dead loop freezes the preview
				// and reads as a crash). Log once, tolerate transients, stop if persistent.
				if (drawErrors++ === 0) console.error("preview draw() failed:", err);
				if (drawErrors > 120) {
					console.error("preview draw() failing persistently; stopping loop");
					wcRafHandle = null;
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
		}
	}

	/**
	 * Capture the current preview frame as a PNG blob: the full composite
	 * (video + background + zoom + blur + cursor), so the screenshot matches
	 * what the user sees rather than the raw recording.
	 *
	 * preserveDrawingBuffer is false, so the back buffer is cleared once the JS
	 * task yields. Workaround: draw() synchronously, then drawImage from the GL
	 * canvas to a 2D canvas in the same task (inter-canvas copies preserve the
	 * buffer); toBlob runs against the 2D canvas, which has no such constraint.
	 */
	$effect(() => {
		captureFrame = async () => {
			if (!canvasEl || !gl || webgl2Unsupported) return null;
			try {
				draw();
				const w = canvasEl.width;
				const h = canvasEl.height;
				if (!w || !h) return null;
				const copy = document.createElement("canvas");
				copy.width = w;
				copy.height = h;
				const ctx = copy.getContext("2d");
				if (!ctx) return null;
				// Same-task drawImage from a WebGL canvas captures the current
				// front buffer even when preserveDrawingBuffer is false.
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

	/**
	 * A lost context makes every GL call a silent no-op — no throw, so nothing
	 * downstream notices and the preview freezes for the rest of the session.
	 * The browser only attempts restoration if the loss event is cancelled.
	 */
	function onContextLost(e: Event) {
		e.preventDefault();
		glLost = true;
		stopVideoFrameLoop();
		if (rafHandle !== null) {
			cancelAnimationFrame(rafHandle);
			rafHandle = null;
		}
		// The ring's textures died with the context. Drop it, or decoded frames
		// keep uploading into stale handles — silent INVALID_OPERATION, and the
		// preview stays frozen for the rest of the session.
		frameRing?.dispose();
		frameRing = null;
		gl = null;
		backend = null;
		renderCore = null;
		videoTex = null;
		bgTex = null;
		bgTexReady = false;
		lastBgKey = "";
		hasRenderedFrame = false;
	}

	// A restored context comes back with every GPU object gone, so this is a
	// full re-init, not a resume.
	function onContextRestored() {
		glLost = false;
		initGL();
		// initGL rebuilds the shader/textures it owns; the ring is sized from the
		// source, so it needs its own rebuild. Until the next decode lands, draw()
		// finds an empty ring and falls back to the <video> frame rather than
		// showing black.
		if (mbSource) rebuildFrameRing(mbSource.width, mbSource.height);
		// onContextLost cleared lastBgKey, so the background texture is gone;
		// reload it here since the draw loop no longer does it per frame.
		void loadBackgroundIfNeeded();
		requestRedraw();
		// A recovery deferred while the context was lost can run now the GL is back.
		if (mbRecoverPending) runMbRecover();
		if (store.isPlaying) startVideoFrameLoop();
	}

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

	//  Lifecycle & reactive wiring
	onMount(() => {
		initGL();
		canvasEl?.addEventListener("webglcontextlost", onContextLost);
		canvasEl?.addEventListener("webglcontextrestored", onContextRestored);
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
		canvasEl?.removeEventListener("webglcontextlost", onContextLost);
		canvasEl?.removeEventListener("webglcontextrestored", onContextRestored);
		document.removeEventListener("visibilitychange", onVisibilityChange);
		dprQuery?.removeEventListener("change", onDprChange);
		stopVideoFrameLoop();
		if (rafHandle !== null) cancelAnimationFrame(rafHandle);
		clearTimeout(mbRecoverTimer);
		smoother?.dispose();
		smoother = null;
		mbSource?.dispose();
		mbSource = null;
		frameRing?.dispose();
		frameRing = null;
		if (gl) {
			if (videoTex) gl.deleteTexture(videoTex);
			if (bgTex) gl.deleteTexture(bgTex);
			backend?.dispose();
			// This component remounts on every editor open, and reclaiming a
			// context is GC-timed. Chromium allows ~16 live contexts and
			// force-loses the OLDEST when it hits the cap — which would kill a
			// live editor's preview. Release ours deterministically instead.
			gl.getExtension("WEBGL_lose_context")?.loseContext();
			gl = null;
		}
	});

	function scheduleMbRecover() {
		clearTimeout(mbRecoverTimer);
		mbHealthyFrames = 0;
		mbRecoverTimer = setTimeout(runMbRecover, MB_RECOVER_DELAY_MS);
	}

	// Re-create the MediaBunny source after a transient failure. Deferred until
	// the GL context is back (onContextRestored re-fires this), or the ring would
	// be allocated on a dead context.
	function runMbRecover() {
		mbRecoverTimer = undefined;
		if (!videoSrc) return;
		if (glLost || !gl) {
			mbRecoverPending = true;
			return;
		}
		mbRecoverPending = false;
		loadedMbSrc = "";
		mbRecoverNonce++;
	}

	// MediaBunny frame source (re)created when the media src changes. Owns its own
	// worker + decoder; disposed and rebuilt per source. A decode failure (e.g.
	// an unsupported codec — see `unsupported-formats.ts` in @recast/media) leaves
	// mbSource null so draw() falls back to the <video> element automatically.
	$effect(() => {
		const src = videoSrc;
		// Read so a recovery bump re-runs this effect; the rebuild also resets
		// loadedMbSrc, so the same-src guard below doesn't short-circuit it.
		void mbRecoverNonce;
		// No src: tear down any live engine and fall back to the <video> path.
		if (!src) {
			clearTimeout(mbRecoverTimer);
			mbRecoverTimer = undefined;
			mbRecoverPending = false;
			mbRecoverAttempts = 0;
			if (mbSource) {
				mbSource.dispose();
				mbSource = null;
			}
			frameRing?.dispose();
			frameRing = null;
			mbReady = false;
			webcodecsActive = false;
			loadedMbSrc = "";
			picClock.pause();
			requestRedraw();
			return;
		}
		if (src === loadedMbSrc) return;
		loadedMbSrc = src;
		mbReady = false;
		mbFallbackNotified = false;
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
					frameRing?.put(frame, tsUs);
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
					frameRing?.dispose();
					frameRing = null;
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
						max_upload_ms: Math.round((frameRing?.uploadStats.maxMs ?? 0) * 100) / 100,
						avg_upload_ms: Math.round((frameRing?.uploadStats.avgMs ?? 0) * 100) / 100,
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
				picClock.setDuration(
					originalToOutput(store.timeMap, store.outPoint),
				);
				picClock.seek(
					originalToOutput(store.timeMap, videoEl?.currentTime ?? 0),
				);
				if (store.isPlaying) picClock.play();
				requestRedraw();
			})
			.catch((err) => {
				console.warn(
					"MediaBunny source unavailable; using <video> fallback:",
					err,
				);
				// We fall back to <video>, so the previous source's ring is dead
				// weight — 16 textures, ~133MB of VRAM at 1080p, never sampled
				// again. The mid-playback onError path already did this; only
				// creation-time failure missed it.
				frameRing?.dispose();
				frameRing = null;
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
		if (store.isPlaying) {
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
				picClock.setDuration(
					originalToOutput(store.timeMap, store.outPoint),
				);
				// Restart from the top if we'd just finished; otherwise resume from
				// the current transport position. (Seeding blindly from the <video>
				// time parked the clock at the end on replay → stuck frame.)
				picClock.seek(
					wasAtEnd
						? 0
						: originalToOutput(store.timeMap, videoEl?.currentTime ?? 0),
				);
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
	<AnnotationStatusRail {store} />
	<div
		bind:this={previewRectEl}
		data-annotations-active={isAnnotationActive}
		class="group/preview relative inline-block rounded-[inherit] outline-2 outline-offset-2 outline-transparent transition-[box-shadow,outline-color] duration-200 ease-out motion-reduce:transition-none data-[annotations-active=true]:outline-primary/30 data-[annotations-active=true]:shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-primary)_25%,transparent)]"
	>
		<canvas
			bind:this={canvasEl}
			class="block max-h-full max-w-full transition-opacity duration-200 ease-out motion-reduce:transition-none group-data-[annotations-active=true]/preview:opacity-90"
		></canvas>
		{#if glLost}
			<!-- Recoverable: the browser restores the context once the driver
			     settles, so this is a wait, not a dead end. -->
			<div
				class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 bg-background/95 p-6 text-center"
				role="status"
				aria-live="polite"
			>
				<div class="text-sm font-semibold text-foreground">Restoring preview</div>
				<p class="max-w-md text-xs leading-relaxed text-muted-foreground">
					The graphics driver reset. The preview will come back on its own — your
					recording and edits are unaffected.
				</p>
			</div>
		{/if}
		{#if webgl2Unsupported}
			<!-- Actionable message instead of a blank canvas: reads as a
			     graphics-driver issue, not a broken app. -->
			<div
				class="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 bg-background/95 p-6 text-center"
				role="alert"
			>
				<div class="text-sm font-semibold text-foreground">
					Preview unavailable on this device
				</div>
				<p class="max-w-md text-xs leading-relaxed text-muted-foreground">
					Your graphics driver doesn't expose WebGL2, which Recast's preview needs.
					Updating your GPU driver (NVIDIA / AMD / Intel) usually fixes this. Export still works, since it uses FFmpeg directly.
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
			compositeCanvasEl={blurMirrorEl ?? canvasEl}
		/>
		<TextAnnotationLayer {store} {videoEl} targetEl={previewRectEl} />
		<div class="contents transition-opacity duration-200 ease-out motion-reduce:transition-none group-data-[annotations-active=true]/preview:opacity-55">
			<FocusOverlay {store} {videoEl} targetEl={previewRectEl} />
		</div>
		{#if svgCursor.visible}
			{@const style = resolveCursorSprite(svgCursor.styleId)}
			{@const stateKey = svgCursor.pressed
				? svgCursor.dragging
					? "drag"
					: svgCursor.right
						? "rightPress"
						: "press"
				: "rest"}
			{@const cursorSrc = resolveCursorDataUrl(svgCursor.styleId, stateKey)}{#if style && cursorSrc}
			{@const hot = cursorSpriteHotspot(style, stateKey)}
			{@const hotPctX = (hot.x / 64) * 100}
			{@const hotPctY = (hot.y / 64) * 100}
			<!-- Custom SVG cursor. Wrapper owns left/top/width/opacity (per-frame
			     motion + visibility ramp). Inner img owns the press transform:
			     `scale` is computed in JS per frame, NOT a CSS transition; a
			     transition would lag the impact and desync from the audio.
			     transform-origin = hotspot keeps the cursor tip pinned. -->
			<div
				class="pointer-events-none absolute"
				style="
					left: {(svgCursor.canvasX / svgCursor.compW) * 100}%;
					top: {(svgCursor.canvasY / svgCursor.compH) * 100}%;
					width: {(svgCursor.spritePx / svgCursor.compW) * 100}%;
					opacity: {svgCursor.alpha};
				"
			>
				<img
					src={cursorSrc}
					alt=""
					draggable="false"
					class="block w-full will-change-transform"
					style="
						transform: translate(-{hotPctX}%, -{hotPctY}%) scale({svgCursor.scale});
						transform-origin: {hotPctX}% {hotPctY}%;
						filter: drop-shadow(0 1px 1.5px rgb(0 0 0 / 0.5));
					"
				/>
			</div>
			{/if}
		{/if}
		<!-- Above the cursor SVG so the bubble isn't clipped behind a cursor in
		     its corner. Owns its own video element, synced via store.currentTime.
		     TODO(camera-recording): gated behind CAMERA_OVERLAY_UI_ENABLED. See
		     apps/desktop/docs/camera-recording-todo.md. -->
		{#if CAMERA_OVERLAY_UI_ENABLED}
		<CameraOverlay
			{store}
			{videoEl}
			{cameraSrc}
			targetEl={previewRectEl}
			previewTime={smoothPreviewTime}
		/>
		{/if}
		<CaptionOverlay {store} />
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

	{#if !isReady}
		<div class="pointer-events-none absolute inset-0 flex items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" />
			<span>Loading preview</span>
		</div>
	{/if}
</div>
