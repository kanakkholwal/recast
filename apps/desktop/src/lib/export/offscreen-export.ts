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
 * Not yet handled here (follow-ups): image backgrounds and the DOM-overlay passes
 * (cursor sprite, camera, captions, annotations) need textures wired in — color
 * and gradient backgrounds + the main pass render today.
 */

import {
	ALL_FORMATS,
	BufferTarget,
	CanvasSource,
	Input,
	Mp4OutputFormat,
	Output,
	UrlSource,
	VideoSampleSink,
	type VideoEncodingConfig,
} from "@recast/media/mediabunny";
import { RenderCore } from "../../components/editor/render-core";
import { WebGL2Backend } from "../../components/editor/webgl2-backend";
import type { FrameInput } from "../../components/editor/frame-params";
import { exportFrameCount, exportFrameTime } from "./browser-export-plan";

export interface OffscreenExportOptions {
	/** Source video URL (range-streamed for random access). */
	videoUrl: string;
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
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** Render + encode the timeline in the browser; resolves with a video-only mp4. */
export async function renderTimelineToVideo(opts: OffscreenExportOptions): Promise<Uint8Array> {
	const canvas = new OffscreenCanvas(opts.width, opts.height);
	// preserveDrawingBuffer so CanvasSource can read the frame after render (this
	// is offline, so the perf cost of keeping the back buffer doesn't matter).
	const gl = canvas.getContext("webgl2", {
		alpha: false,
		antialias: false,
		premultipliedAlpha: false,
		preserveDrawingBuffer: true,
	});
	if (!gl) throw new Error("WebGL2 unavailable for export");
	const backend = WebGL2Backend.create(gl);
	const renderCore = new RenderCore(backend);

	const input = new Input({ source: new UrlSource(opts.videoUrl), formats: ALL_FORMATS });
	const output = new Output({ format: new Mp4OutputFormat(), target: new BufferTarget() });
	try {
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new Error("no video track to export");
		const sink = new VideoSampleSink(track);

		const source = new CanvasSource(canvas, opts.encodingConfig);
		output.addVideoTrack(source);
		await output.start();

		const frames = exportFrameCount(opts.fps, opts.outputDurationSec);
		const frameDur = opts.fps > 0 ? 1 / opts.fps : 0;
		for (let i = 0; i < frames; i++) {
			if (opts.signal?.aborted) throw new Error("export cancelled");
			const outputSec = exportFrameTime(i, opts.fps);
			const { input: frameInput, originalSec } = opts.frameAt(i, outputSec);
			const sample = await sink.getSample(Math.max(0, originalSec));
			if (sample) {
				const vf = sample.toVideoFrame();
				backend.uploadFrame(vf);
				vf.close();
				sample.close();
			}
			renderCore.renderFrame(frameInput, { backgroundTex: null });
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
		input.dispose();
		backend.dispose();
	}
}
