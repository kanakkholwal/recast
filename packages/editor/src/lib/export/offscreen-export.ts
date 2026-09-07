/**
 * Offline export renderer: composites every output frame through the SAME wasm
 * engine the preview draws with and WebCodecs-encodes it to a video-only mp4 via
 * MediaBunny. Rust FFmpeg then muxes the processed audio in (`-c:v copy`).
 *
 * There is one compositor, so preview and export cannot diverge. Everything the
 * old GL path drew as a separate overlay pass — the cursor, the camera bubble,
 * annotations, captions — is inside the engine, evaluated from the same scene.
 *
 * Deterministic pull loop (every frame, no drops), respecting encoder
 * backpressure via `await source.add(...)`.
 */

import { detectBackend, type NavigatorLike, PreviewEngine } from "@recast/engine";
import { type MediaRef, toMediaRef } from "@recast/media";
import {
	ALL_FORMATS,
	BufferTarget,
	CanvasSource,
	Input,
	Mp4OutputFormat,
	mediaRefSource,
	Output,
	type VideoEncodingConfig,
	VideoSampleSink,
} from "@recast/media/mediabunny";
import type { CursorSpriteUpload } from "../playback/engine-driver";
import { outputToOriginal, type TimeMap } from "../timeline/time-map";
import { exportFrameCount, exportFrameTime } from "./browser-export-plan";

/** Everything the engine needs that is not already in the scene. */
export interface EngineAssets {
	/** Decoded image/wallpaper background; null for colour and gradient. */
	backgroundImage?: ImageBitmap | null;
	/** Pointer sprites by slot. Empty leaves the engine drawing its dot. */
	cursorSprites?: readonly CursorSpriteUpload[];
	/** Smoothed cursor track, in the shape `setCursorTrack` takes. */
	cursorTrack?: unknown | null;
	/** Word timings for burned captions; null leaves captions off. */
	captionTrack?: unknown | null;
	/** TTF bytes for the caption face. The engine shapes with rustybuzz, which
	 *  cannot read the woff2 the DOM uses. */
	captionFont?: Uint8Array | null;
	/** Decoded assets for image annotations, by path. */
	annotationImages?: ReadonlyArray<[string, ImageBitmap]>;
}

/** The camera bubble's own stream. Placement comes from the scene. */
export interface CameraExportInputs {
	url: MediaRef | string;
	/** Milliseconds the camera track starts after video frame 0, measured at
	 *  capture. Without it the bubble is sampled at the screen's timestamps and
	 *  the face lags the action by however long the recorder took to come up. */
	offsetMs?: number;
}

export interface OffscreenExportOptions {
	/** Source video. A `blob` ref streams off a File; a `url` ref range-requests. */
	videoUrl: MediaRef | string;
	/** The scene the engine evaluates: a `RenderState` or a `Scene`. */
	scene: unknown;
	/** Output-to-original mapping (cuts and per-segment speed). */
	timeMap: TimeMap;
	/** Source dimensions, which set the composition's aspect. */
	sourceWidth: number;
	sourceHeight: number;
	/** Composited output size (px). */
	width: number;
	height: number;
	fps: number;
	/** Total OUTPUT duration (seconds) after cuts/speed. */
	outputDurationSec: number;
	encodingConfig: VideoEncodingConfig;
	assets?: EngineAssets;
	camera?: CameraExportInputs | null;
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

// MediaBunny's decoder close isn't idempotent, so a double close throws a detached rejection; swallow just that benign one.
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
 * Build the engine's canvas.
 *
 * `CanvasSource.add` captures the canvas synchronously, and a WebGL2 drawing
 * buffer is cleared at the end of the task that drew it. Creating the context
 * HERE with `preserveDrawingBuffer` is what keeps the frame readable: a later
 * `getContext("webgl2")` returns this same context whatever attributes it asks
 * for, so wgpu adopts it. WebGPU keeps its presented image by spec, so it needs
 * nothing. The cost of preserving the buffer does not matter offline.
 */
async function exportCanvas(width: number, height: number): Promise<OffscreenCanvas> {
	const canvas = new OffscreenCanvas(width, height);
	const backend = await detectBackend(globalThis.navigator as NavigatorLike, "auto").catch(
		() => "webgl2" as const,
	);
	if (backend === "webgl2") {
		canvas.getContext("webgl2", {
			alpha: false,
			antialias: false,
			premultipliedAlpha: false,
			preserveDrawingBuffer: true,
			powerPreference: "high-performance",
		});
	}
	return canvas;
}

function applyAssets(engine: PreviewEngine, assets: EngineAssets | undefined) {
	if (!assets) return;
	if (assets.backgroundImage) engine.setBackgroundImage(assets.backgroundImage);
	for (const sprite of assets.cursorSprites ?? []) {
		engine.setCursorSprite(sprite.slot, sprite.image, sprite.hotspot);
	}
	for (const [path, image] of assets.annotationImages ?? []) {
		engine.setAnnotationImage(path, image);
	}
	if (assets.cursorTrack) engine.setCursorTrack(assets.cursorTrack);
	// The font must land before the track: layout measures glyphs, so a fallback face keeps the fallback's line breaks.
	if (assets.captionFont && !engine.setCaptionFont(assets.captionFont)) {
		console.warn("export: the caption font was rejected; falling back to the bundled face");
	}
	if (assets.captionTrack) engine.setCaptionTrack(assets.captionTrack);
}

/** Render + encode the timeline in the browser; resolves with an mp4. */
export async function renderTimelineToVideo(opts: OffscreenExportOptions): Promise<Uint8Array> {
	installClosedCodecGuard();
	const canvas = await exportCanvas(opts.width, opts.height);
	const engine = await PreviewEngine.create(canvas);

	const input = new Input({
		source: mediaRefSource(toMediaRef(opts.videoUrl)),
		formats: ALL_FORMATS,
	});
	const output = new Output({ format: new Mp4OutputFormat(), target: new BufferTarget() });
	let camInput: Input | null = null;
	// Wound down in `finally`: abandoning an iterator mid-iteration leaves its decoder and pre-decoded frames alive.
	const iterators: AsyncGenerator<unknown, void, unknown>[] = [];

	try {
		engine.setSourceSize(opts.sourceWidth, opts.sourceHeight);
		// Before the scene: the time map is what output time MEANS, or a scene with a missing cut resolves on the old axis.
		engine.setTimeMap(opts.timeMap);
		engine.setScene(opts.scene);
		engine.setCanvasSize(opts.width, opts.height);
		applyAssets(engine, opts.assets);

		const screenLayer = engine.screenLayerId;
		if (screenLayer === undefined) throw new Error("export: the scene has no screen layer");
		const cameraLayer = engine.cameraLayerId;
		// One frame at a time: this loop binds what it just uploaded, so a playback-depth ring would only hold VRAM.
		engine.setLayerRingCapacity(screenLayer, 2);
		if (cameraLayer !== undefined) engine.setLayerRingCapacity(cameraLayer, 2);

		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new Error("no video track to export");
		const sink = new VideoSampleSink(track);

		// Camera bubble: its own decoder, sampled at the same original time.
		let camSink: VideoSampleSink | null = null;
		if (opts.camera && cameraLayer !== undefined) {
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

		const frames = exportFrameCount(opts.fps, opts.outputDurationSec);
		const frameDur = opts.fps > 0 ? 1 / opts.fps : 0;

		// `getSample(t)` builds a FRESH decoder per call and re-decodes the GOP; `samplesAtTimestamps` decodes each packet once.
		const originalAt = (i: number) => outputToOriginal(opts.timeMap, exportFrameTime(i, opts.fps));
		function* sampleTimes(shiftSec = 0): Generator<number> {
			for (let i = 0; i < frames; i++) yield Math.max(0, originalAt(i) - shiftSec);
		}
		const mainSamples = sink.samplesAtTimestamps(sampleTimes());
		const camShiftSec = (opts.camera?.offsetMs ?? 0) / 1000;
		const camSamples = camSink ? camSink.samplesAtTimestamps(sampleTimes(camShiftSec)) : null;
		// Releases the decoders when the loop exits early (abort, GPU loss).
		iterators.push(mainSamples);
		if (camSamples) iterators.push(camSamples);

		// A GPU reset makes every draw a silent no-op, finalising an mp4 that is one frozen frame with no error anywhere.
		let drewSomething = false;
		for (let i = 0; i < frames; i++) {
			if (opts.signal?.aborted) throw new Error("export cancelled");
			const outputSec = exportFrameTime(i, opts.fps);
			const originalUs = Math.max(0, Math.round(originalAt(i) * 1e6));

			const sample = (await mainSamples.next()).value;
			if (sample) {
				const vf = sample.toVideoFrame();
				try {
					engine.putLayerFrame(screenLayer, vf, originalUs);
				} finally {
					// An upload throw must not strand the frame: a retained VideoFrame silently starves the decoder.
					vf.close();
					sample.close();
				}
			}
			// Floor 0: this loop uploads exactly the frame it wants, so no older ring segment needs excluding.
			engine.bindLayerFrame(screenLayer, originalUs, 0);

			if (camSamples && cameraLayer !== undefined) {
				const cs = (await camSamples.next()).value;
				if (cs) {
					const cvf = cs.toVideoFrame();
					try {
						engine.putLayerFrame(cameraLayer, cvf, originalUs);
					} finally {
						cvf.close();
						cs.close();
					}
				}
				engine.bindLayerFrame(cameraLayer, originalUs, 0);
			}

			const drawn = engine.render(outputSec);
			if (drawn === 0 && drewSomething) {
				throw new Error("export failed: the compositor stopped drawing mid-render");
			}
			drewSomething ||= drawn > 0;

			await source.add(outputSec, frameDur); // backpressure
			opts.onProgress?.((i + 1) / frames);
		}

		await output.finalize();
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
		for (const it of iterators) await it.return(undefined).catch(() => undefined);
		input.dispose();
		camInput?.dispose();
		engine.destroy();
	}
}
