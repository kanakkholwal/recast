/**
 * Offline export renderer (Phase 4, browser half): composites every output frame
 * through the SAME RenderCore the preview uses and WebCodecs-encodes it to a
 * video-only mp4 via MediaBunny. Rust FFmpeg then muxes the processed audio in
 * (`-c:v copy`), so there is ONE compositor — preview and export can't diverge.
 *
 * Deterministic pull loop (every frame, no drops), respecting encoder
 * backpressure via `await source.add(...)`. The caller supplies the per-frame
 * scene (`frameAt`) so the RenderState→FrameInput mapping stays testable and out
 * of this imperative glue.
 *
 * Not yet handled here (follow-ups): caption + annotation passes. Background
 * (colour/gradient/image), sprite cursor, and the camera bubble render today.
 */

import {
	ALL_FORMATS,
	AudioBufferSink,
	AudioBufferSource,
	BufferTarget,
	CanvasSource,
	Input,
	mediaRefSource,
	Mp4OutputFormat,
	Output,
	VideoSampleSink,
	type VideoEncodingConfig,
} from "@recast/media/mediabunny";
import { type MediaRef, type Region, toMediaRef } from "@recast/media";
import { type AudioSpan, applyFade, applyGain, planAudioSpans, timeStretch } from "./audio-export";
import { RenderCore, type RenderPass, type RenderPassContext } from "../../components/render-core";
import { WebGL2Backend } from "../../components/webgl2-backend";
import type { FrameGeometry, FrameInput } from "../../components/frame-params";
import type { CameraOverlayShape, CameraPlacement } from "../../stores/editor-store.svelte";
import { exportFrameCount, exportFrameTime } from "./browser-export-plan";
import { bubbleCornerRadiusPx, cameraBubbleRect, coverUvRect } from "./camera-overlay-export";

/** An overlay bound to the export GL context: its per-frame pass + a disposer for
 *  the textures it uploaded. Built by a factory so browser-export owns the data
 *  (sprites, bitmaps) and this renderer stays generic. */
export interface ExportOverlay {
	readonly pass: RenderPass;
	dispose(): void;
}
export type ExportOverlayFactory = (backend: WebGL2Backend) => ExportOverlay;

/** Blur env handed to the annotation-layer draw: the composited GL frame to
 *  sample + a resizable scratch, so blur annotations blur the real frame. */
export interface BlurLayerEnv {
	composite: CanvasImageSource;
	srcW: number;
	srcH: number;
	getScratch: (
		w: number,
		h: number,
	) => { ctx: OffscreenCanvasRenderingContext2D; canvas: CanvasImageSource } | null;
}

/** Camera bubble inputs: its own decoded stream (sampled at the same original
 *  time as the main video) + the resolved per-frame placement. The bubble draws
 *  on top of all other overlays, matching the preview's stacking. */
export interface CameraExportInputs {
	/** Camera stream. */
	url: MediaRef | string;
	geom: FrameGeometry;
	shape: CameraOverlayShape;
	cornerRadius: number | undefined;
	mirror: boolean;
	/** Effective placement at original time `t` (base → keyframes → zoom-follow). */
	placementAt: (originalSec: number) => CameraPlacement;
	/** Milliseconds the camera track starts after video frame 0, measured at
	 *  capture. Without it the bubble is sampled at the screen's timestamps and
	 *  the face lags the action by however long the recorder took to come up. */
	offsetMs?: number;
}

export interface OffscreenExportOptions {
	/** Source video. A `blob` ref streams off a File; a `url` ref range-requests. */
	videoUrl: MediaRef | string;
	/** Composited output size (px). */
	width: number;
	height: number;
	fps: number;
	/** Total OUTPUT duration (seconds) after cuts/speed. */
	outputDurationSec: number;
	encodingConfig: VideoEncodingConfig;
	/** Scene for output frame `index`: the FrameInput to composite and the
	 *  ORIGINAL source time to sample the decoded frame at. */
	frameAt: (index: number, outputSec: number) => { input: FrameInput; originalSec: number };
	/** Decoded image/wallpaper background, uploaded once to a texture; null for
	 *  colour/gradient backgrounds. Must match the scene's `backgroundImageReady`. */
	backgroundImage?: ImageBitmap | null;
	/** Overlay passes (cursor sprite, captions, annotations) drawn after the main
	 *  compositor pass. Built + disposed by the caller's data. */
	overlays?: ExportOverlayFactory[];
	/** Camera bubble (a second decoded stream), drawn on top when present. */
	camera?: CameraExportInputs | null;
	/** Draw the annotation layer for original time `t` into a comp-native 2D ctx.
	 *  Composited below the cursor (matching the preview), above the video. Runs
	 *  AFTER the main pass, so `blur` (the composited GL frame + a scratch) is
	 *  available for blur annotations to sample. */
	annotationLayer?:
		| ((ctx: OffscreenCanvasRenderingContext2D, originalSec: number, blur: BlurLayerEnv) => void)
		| null;
	/** Draw burned captions onto the SAME layer canvas as the annotations but after
	 *  them (matching the preview's overlay order). `originalSec` resolves the
	 *  chunk/highlight; `outputSec` clocks the entrance at viewer-rate. */
	captionLayer?:
		| ((ctx: OffscreenCanvasRenderingContext2D, originalSec: number, outputSec: number) => void)
		| null;
	/** Source audio to carry into the mux. Omitted ⇒ a video-only mp4. */
	audio?: AudioExportInputs | null;
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** The recording's own audio track, warped onto the output timeline by the same
 *  regions the preview plays. */
export interface AudioExportInputs {
	/** Where to read the audio from; usually the same ref as the video. */
	source: MediaRef | string;
	/** Kept regions (trim + cuts + per-segment speed), in original time. */
	regions: readonly Region[];
	/** Master volume 0..1 and the fade envelope, mirroring the preview. */
	gain?: number;
	fadeInSec?: number;
	fadeOutSec?: number;
}

// MediaBunny's VideoDecoderWrapper.close isn't idempotent: on teardown the
// samples-generator's cleanup AND our `input.dispose()` can both close the same
// decoder, and the loser throws "Cannot call 'close' on a closed codec" as a
// detached rejection we can't try/catch. Closing an already-closed codec is
// harmless (the decode finished), so swallow just that one benign rejection.
// Idempotent + context-scoped (dies with the worker; app-wide on the main thread,
// where the message is specific enough to never hide a real bug).
let codecCloseGuardInstalled = false;
function installClosedCodecGuard() {
	if (codecCloseGuardInstalled) return;
	const g = globalThis as {
		addEventListener?: (t: string, h: (e: PromiseRejectionEvent) => void) => void;
	};
	if (!g.addEventListener) return;
	codecCloseGuardInstalled = true;
	g.addEventListener("unhandledrejection", (e) => {
		const msg = (e as PromiseRejectionEvent).reason?.message;
		if (typeof msg === "string" && msg.includes("closed codec")) e.preventDefault();
	});
}

/**
 * Decode the source's audio for `spans` and append it to `source` in output
 * order. Streams span by span so a long recording never holds its whole PCM.
 */
async function encodeAudioSpans(
	audio: AudioExportInputs,
	spans: readonly AudioSpan[],
	totalOutputSec: number,
	source: AudioBufferSource,
	signal?: AbortSignal,
): Promise<void> {
	const input = new Input({
		source: mediaRefSource(toMediaRef(audio.source)),
		formats: ALL_FORMATS,
	});
	try {
		const track = await input.getPrimaryAudioTrack();
		if (!track) return;
		const sink = new AudioBufferSink(track);
		const gain = audio.gain ?? 1;
		const fadeIn = audio.fadeInSec ?? 0;
		const fadeOut = audio.fadeOutSec ?? 0;
		for (const span of spans) {
			if (signal?.aborted) throw new Error("export cancelled");
			let outCursor = span.outputStart;
			for await (const { buffer, timestamp } of sink.buffers(span.sourceStart, span.sourceEnd)) {
				if (signal?.aborted) throw new Error("export cancelled");
				// Clip to the span: a decoded buffer can overhang both ends.
				const startInBuf = Math.max(0, span.sourceStart - timestamp);
				const endInBuf = Math.min(buffer.duration, span.sourceEnd - timestamp);
				if (endInBuf <= startInBuf) continue;
				const from = Math.floor(startInBuf * buffer.sampleRate);
				const to = Math.floor(endInBuf * buffer.sampleRate);
				const frames = to - from;
				if (frames <= 0) continue;

				const channels: Float32Array[] = [];
				for (let c = 0; c < buffer.numberOfChannels; c++) {
					const raw = buffer.getChannelData(c).subarray(from, to);
					const warped = timeStretch(new Float32Array(raw), span.rate, buffer.sampleRate);
					applyGain(warped, gain);
					applyFade(warped, buffer.sampleRate, totalOutputSec, fadeIn, fadeOut, outCursor);
					channels.push(warped);
				}
				const outFrames = channels[0]?.length ?? 0;
				if (outFrames === 0) continue;
				const out = new AudioBuffer({
					length: outFrames,
					numberOfChannels: channels.length,
					sampleRate: buffer.sampleRate,
				});
				for (let c = 0; c < channels.length; c++) {
					out.copyToChannel(channels[c] as Float32Array<ArrayBuffer>, c);
				}
				await source.add(out);
				outCursor += outFrames / buffer.sampleRate;
			}
		}
	} finally {
		input.dispose();
	}
}

/** Render + encode the timeline in the browser; resolves with an mp4. */
export async function renderTimelineToVideo(opts: OffscreenExportOptions): Promise<Uint8Array> {
	installClosedCodecGuard();
	const canvas = new OffscreenCanvas(opts.width, opts.height);
	// preserveDrawingBuffer so CanvasSource can read the frame after render (this
	// is offline, so the perf cost of keeping the back buffer doesn't matter).
	const gl = canvas.getContext("webgl2", {
		alpha: false,
		antialias: false,
		premultipliedAlpha: false,
		preserveDrawingBuffer: true,
		powerPreference: "high-performance",
	});
	if (!gl) throw new Error("WebGL2 unavailable for export");
	const backend = WebGL2Backend.create(gl);
	const overlays = (opts.overlays ?? []).map((make) => make(backend));

	// Annotation layer: its own comp-native 2D canvas, composited (below the
	// cursor) via a pass ordered before the overlay passes.
	let annoCanvas: OffscreenCanvas | null = null;
	let annoCtx: OffscreenCanvasRenderingContext2D | null = null;
	let scratchCanvas: OffscreenCanvas | null = null;
	const getScratch = (w: number, h: number) => {
		const cw = Math.max(1, w);
		const ch = Math.max(1, h);
		if (!scratchCanvas) scratchCanvas = new OffscreenCanvas(cw, ch);
		if (scratchCanvas.width !== cw || scratchCanvas.height !== ch) {
			scratchCanvas.width = cw;
			scratchCanvas.height = ch;
		}
		const c = scratchCanvas.getContext("2d");
		return c ? { ctx: c, canvas: scratchCanvas as CanvasImageSource } : null;
	};
	const blurEnv: BlurLayerEnv = {
		composite: canvas,
		srcW: opts.width,
		srcH: opts.height,
		getScratch,
	};
	// One comp-native 2D layer carries annotations AND burned captions (captions
	// drawn on top, matching the preview's overlay order).
	const passes: RenderPass[] = [];
	if (opts.annotationLayer || opts.captionLayer) {
		annoCanvas = new OffscreenCanvas(opts.width, opts.height);
		annoCtx = annoCanvas.getContext("2d");
		passes.push({
			name: "annotation-layer",
			render(be, params, ctx) {
				if (!ctx.annotationTex) return;
				const [cw, ch] = params.uniforms.canvasSize;
				be.drawSprite(ctx.annotationTex, { x: 0, y: 0, w: cw, h: ch });
			},
		});
	}
	passes.push(...overlays.map((o) => o.pass));
	const renderCore = new RenderCore(backend, passes);

	const input = new Input({
		source: mediaRefSource(toMediaRef(opts.videoUrl)),
		formats: ALL_FORMATS,
	});
	const output = new Output({ format: new Mp4OutputFormat(), target: new BufferTarget() });
	let camInput: Input | null = null;
	// Sample iterators to wind down in `finally`: abandoning one mid-iteration
	// leaves its decoder and pre-decoded frames alive.
	const iterators: AsyncGenerator<unknown, void, unknown>[] = [];

	// A lost context can strand `source.add` (the encoder can't read the dead
	// canvas), hanging the export forever at N%. Reject the encoder awaits the
	// instant the event fires so it fails cleanly instead of stalling.
	let rejectOnLost: (e: Error) => void = () => {};
	const lostPromise = new Promise<never>((_, reject) => {
		rejectOnLost = reject;
	});
	lostPromise.catch(() => {}); // always "handled" so a late loss isn't an unhandled rejection
	const onContextLost = (e: Event) => {
		e.preventDefault();
		rejectOnLost(new Error("export failed: GPU context lost mid-render"));
	};
	canvas.addEventListener("webglcontextlost", onContextLost);

	try {
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new Error("no video track to export");
		const sink = new VideoSampleSink(track);

		// Camera bubble: its own decoder, sampled at the same original time.
		let camSink: VideoSampleSink | null = null;
		if (opts.camera) {
			camInput = new Input({
				source: mediaRefSource(toMediaRef(opts.camera.url)),
				formats: ALL_FORMATS,
			});
			const camTrack = await camInput.getPrimaryVideoTrack();
			if (camTrack) camSink = new VideoSampleSink(camTrack);
		}

		const source = new CanvasSource(canvas, opts.encodingConfig);
		output.addVideoTrack(source);
		await output.start();

		const backgroundTex = opts.backgroundImage
			? backend.uploadBackground(opts.backgroundImage)
			: null;

		const frames = exportFrameCount(opts.fps, opts.outputDurationSec);
		const frameDur = opts.fps > 0 ? 1 / opts.fps : 0;

		// `sink.getSample(t)` builds a FRESH VideoDecoder per call and decodes from
		// the preceding keyframe, so calling it per frame re-decoded each GOP once
		// per output frame in it (~15x at our GOP of fps/2, ~60x for imported
		// footage). `samplesAtTimestamps` decodes each packet at most once.
		//
		// Both sinks walk the same timestamps, so the scene is evaluated once per
		// frame and memoized; the sinks pre-decode ahead, so entries are dropped as
		// the render loop passes them and only the lookahead window stays resident.
		const scenes = new Map<number, { input: FrameInput; originalSec: number }>();
		const sceneAt = (i: number) => {
			let scene = scenes.get(i);
			if (!scene) {
				scene = opts.frameAt(i, exportFrameTime(i, opts.fps));
				scenes.set(i, scene);
			}
			return scene;
		};
		function* sampleTimes(shiftSec = 0): Generator<number> {
			for (let i = 0; i < frames; i++) {
				yield Math.max(0, sceneAt(i).originalSec - shiftSec);
			}
		}
		const mainSamples = sink.samplesAtTimestamps(sampleTimes());
		const camShiftSec = (opts.camera?.offsetMs ?? 0) / 1000;
		const camSamples = camSink ? camSink.samplesAtTimestamps(sampleTimes(camShiftSec)) : null;
		// Releases the decoders when the loop exits early (abort, GPU loss).
		iterators.push(mainSamples);
		if (camSamples) iterators.push(camSamples);
		// A layer draw that throws must NOT abort the whole export (which would
		// silently fall back to the Rust path and drop the overlay with no signal).
		// Log the first failure per layer and keep rendering the rest of the frame.
		let annoErrLogged = false;
		let capErrLogged = false;
		for (let i = 0; i < frames; i++) {
			if (opts.signal?.aborted) throw new Error("export cancelled");
			// A lost context (GPU TDR / driver reset) turns every upload+draw into a
			// silent no-op, which would finalize a black-from-here mp4 with no error.
			// Fail loudly instead so the export is retried, not silently corrupted.
			if (gl.isContextLost()) throw new Error("export failed: GPU context lost mid-render");
			const outputSec = exportFrameTime(i, opts.fps);
			const { input: frameInput, originalSec } = sceneAt(i);
			const sample = (await mainSamples.next()).value;
			if (sample) {
				const vf = sample.toVideoFrame();
				try {
					backend.uploadFrame(vf);
				} finally {
					// An upload throw (lost context is expected here) must not strand
					// the frame — a retained VideoFrame silently starves the decoder.
					vf.close();
					sample.close();
				}
			}
			// The annotation + caption layer is drawn in `afterMain` (after the GL
			// main pass) so blur annotations can sample the just-composited frame.
			const ctx: RenderPassContext = { backgroundTex, annotationTex: null };
			renderCore.renderFrame(frameInput, ctx, () => {
				if (!annoCtx || !annoCanvas) return;
				// Flush so blur reads the completed main-pass frame off the GL canvas.
				if (opts.annotationLayer) backend.finish();
				annoCtx.clearRect(0, 0, annoCanvas.width, annoCanvas.height);
				if (opts.annotationLayer) {
					try {
						opts.annotationLayer(annoCtx, originalSec, blurEnv);
					} catch (e) {
						if (!annoErrLogged) {
							annoErrLogged = true;
							console.error("export: annotation layer draw failed (frames continue)", e);
						}
					}
				}
				if (opts.captionLayer) {
					try {
						opts.captionLayer(annoCtx, originalSec, outputSec);
					} catch (e) {
						if (!capErrLogged) {
							capErrLogged = true;
							console.error("export: caption layer draw failed (frames continue)", e);
						}
					}
				}
				ctx.annotationTex = backend.uploadAnnotation(annoCanvas);
			});

			if (opts.camera && camSamples) {
				const cs = (await camSamples.next()).value;
				if (cs) {
					const cvf = cs.toVideoFrame();
					try {
						const camTex = backend.uploadCamera(cvf);
						const camAspect = cvf.displayHeight > 0 ? cvf.displayWidth / cvf.displayHeight : 1;
						const placement = opts.camera.placementAt(originalSec);
						const rect = cameraBubbleRect(placement, opts.camera.geom, opts.width, opts.height);
						backend.drawSprite(camTex, rect, {
							uvRect: coverUvRect(camAspect, opts.camera.mirror),
							cornerRadiusPx: bubbleCornerRadiusPx(
								opts.camera.shape,
								opts.camera.cornerRadius,
								rect.w,
							),
						});
					} finally {
						cvf.close();
						cs.close();
					}
				}
			}
			// Both sinks have consumed this index by now; drop it so the memo tracks
			// only the lookahead window rather than the whole timeline.
			scenes.delete(i);

			// Race the encoder awaits against context loss — the only awaits that hang
			// when the GL canvas dies (the decoder awaits are WebCodecs, unaffected).
			await Promise.race([source.add(outputSec, frameDur), lostPromise]); // backpressure
			opts.onProgress?.((i + 1) / frames);
		}

		await Promise.race([output.finalize(), lostPromise]);
		const buf = (output.target as BufferTarget).buffer;
		if (!buf) throw new Error("export produced no data");
		return new Uint8Array(buf);
	} catch (err) {
		try {
			if (output.state === "started") await output.cancel();
		} catch {
			/* already torn down */
		}
		throw err;
	} finally {
		rejectOnLost = () => {};
		canvas.removeEventListener("webglcontextlost", onContextLost);
		for (const it of iterators) await it.return(undefined).catch(() => {});
		for (const o of overlays) o.dispose();
		input.dispose();
		camInput?.dispose();
		backend.dispose();
	}
}
