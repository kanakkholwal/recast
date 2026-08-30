import { domToCanvas } from "modern-screenshot";
import { ArrayBufferTarget, Muxer } from "mp4-muxer";
import { type AnimationPreset, propsAtTime, propsToTransform } from "./animation";
import { clipTime } from "./defaults";
import { exportFilter } from "./export";

/** How the timeline maps to preset-local time, so the export matches the
 * looping preview EXACTLY (full timeline length + hold frames before/after the
 * clip). Omitted = sample the preset's own duration end-to-end. */
export interface VideoTimeline {
	duration: number; // total timeline length (ms)
	clipStart: number; // ms where the clip begins
	clipLength: number; // ms the clip spans (stretched)
}

/** Frame count + a frame->preset-time sampler for the given timeline. */
function sampler(preset: AnimationPreset, fps: number, timeline?: VideoTimeline) {
	const span = timeline && timeline.duration > 0 ? timeline.duration : preset.duration;
	const total = Math.max(2, Math.round((span / 1000) * fps));
	const timeAt = (i: number) => {
		const playhead = (i / (total - 1)) * span;
		return timeline
			? clipTime(playhead, timeline.clipStart, timeline.clipLength, preset.duration)
			: playhead;
	};
	return { total, timeAt };
}

/** True when this browser can encode H.264/MP4 via WebCodecs. */
export function canExportVideo(): boolean {
	return typeof VideoEncoder !== "undefined" && typeof VideoFrame !== "undefined";
}

/** True when this browser can record WebM via MediaRecorder + captureStream.
 * This is the dependency-free path (no WebM muxer), used for WebM export. */
export function canExportWebM(): boolean {
	return (
		typeof MediaRecorder !== "undefined" &&
		typeof HTMLCanvasElement !== "undefined" &&
		typeof HTMLCanvasElement.prototype.captureStream === "function" &&
		(MediaRecorder.isTypeSupported?.("video/webm") ?? false)
	);
}

/** Any video export path is available. */
export function canExportAnyVideo(): boolean {
	return canExportVideo() || canExportWebM();
}

/** Even, long-edge-capped output dimensions for a probe canvas. */
function outputDims(probe: HTMLCanvasElement, cap = 1920): { width: number; height: number } {
	const longest = Math.max(probe.width, probe.height);
	const f = longest > cap ? cap / longest : 1;
	return {
		width: Math.max(2, Math.round(probe.width * f)) & ~1,
		height: Math.max(2, Math.round(probe.height * f)) & ~1,
	};
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Pick the best supported WebM codec string for MediaRecorder. */
function webmMime(): string {
	const candidates = ["video/webm;codecs=vp9", "video/webm;codecs=vp8", "video/webm"];
	return candidates.find((c) => MediaRecorder.isTypeSupported?.(c)) ?? "video/webm";
}

/** Drive the framed content's live transform to a given animation time by
 * mutating the same DOM nodes the preview uses, so the captured frame is
 * exactly what the preview shows. */
function applyFrame(persp: HTMLElement, tilt: HTMLElement, preset: AnimationPreset, time: number) {
	const p = propsAtTime(preset, time);
	persp.style.perspective = `${p.perspective}px`;
	tilt.style.transform = propsToTransform(p);
	tilt.style.opacity = String(p.opacity);
}

/**
 * Encode the selected motion preset to an MP4 by snapshotting the real stage at
 * each frame (so 3D perspective, mockups, and overlays all render faithfully),
 * then muxing the WebCodecs H.264 chunks. `onProgress` reports 0..1.
 */
export async function exportVideo(
	stage: HTMLElement,
	preset: AnimationPreset,
	fps: number,
	onProgress?: (progress: number) => void,
	/** Timeline mapping so the export matches the preview (length + holds). */
	timeline?: VideoTimeline,
): Promise<Blob> {
	if (!canExportVideo()) throw new Error("this browser can't encode video (needs WebCodecs)");

	const persp = stage.querySelector<HTMLElement>(".recast-shot-persp");
	const tilt = stage.querySelector<HTMLElement>(".recast-shot-tilt");
	if (!persp || !tilt) throw new Error("load an image before exporting a clip");

	// Preserve the live styles so the editor is untouched afterwards.
	const saved = {
		perspective: persp.style.perspective,
		transform: tilt.style.transform,
		opacity: tilt.style.opacity,
		transition: tilt.style.transition,
	};
	tilt.style.transition = "none";

	// Declared out here so `finally` can close it: every throw path used to leak a scarce hardware encoder session.
	let encoder: VideoEncoder | null = null;

	try {
		// Probe frame 0 to size the output; force even dims, cap the long edge.
		applyFrame(persp, tilt, preset, 0);
		const probe = await domToCanvas(stage, { scale: 2, filter: exportFilter });
		const cap = 1920;
		const longest = Math.max(probe.width, probe.height);
		const f = longest > cap ? cap / longest : 1;
		const width = Math.max(2, Math.round(probe.width * f)) & ~1;
		const height = Math.max(2, Math.round(probe.height * f)) & ~1;

		const out = document.createElement("canvas");
		out.width = width;
		out.height = height;
		const octx = out.getContext("2d");
		if (!octx) throw new Error("could not create a drawing context");

		const muxer = new Muxer({
			target: new ArrayBufferTarget(),
			video: { codec: "avc", width, height },
			fastStart: "in-memory",
		});
		encoder = new VideoEncoder({
			output: (chunk, meta) => muxer.addVideoChunk(chunk, meta),
			error: (e) => {
				throw e;
			},
		});
		encoder.configure({ codec: "avc1.42001f", width, height, bitrate: 6_000_000, framerate: fps });

		// Frame count comes from the clip's wall-clock length while motion samples the preset's full range, so a stretched clip plays slower.
		const { total, timeAt } = sampler(preset, fps, timeline);
		for (let i = 0; i < total; i++) {
			const time = timeAt(i);
			applyFrame(persp, tilt, preset, time);
			const canvas = await domToCanvas(stage, { scale: 2, filter: exportFilter });
			octx.clearRect(0, 0, width, height);
			octx.drawImage(canvas, 0, 0, width, height);
			const frame = new VideoFrame(out, {
				timestamp: Math.round((i * 1_000_000) / fps),
				duration: Math.round(1_000_000 / fps),
			});
			encoder.encode(frame, { keyFrame: i % fps === 0 });
			frame.close();
			onProgress?.((i + 1) / total);
		}

		await encoder.flush();
		muxer.finalize();
		return new Blob([muxer.target.buffer], { type: "video/mp4" });
	} finally {
		// `close()` on an already-closed encoder throws, and a successful flush() still leaves it open.
		if (encoder && encoder.state !== "closed") encoder.close();
		persp.style.perspective = saved.perspective;
		tilt.style.transform = saved.transform;
		tilt.style.opacity = saved.opacity;
		tilt.style.transition = saved.transition;
	}
}

/**
 * Encode the selected motion to a WebM via MediaRecorder (no external muxer).
 * Frames are pre-rendered offline by snapshotting the real stage (so 3D,
 * mockups, and overlays render faithfully), then paced onto a captured canvas
 * stream in real time so the clip length is correct. `onProgress` reports 0..1
 * (first ~60% is rendering, the rest is real-time capture).
 */
export async function exportVideoWebM(
	stage: HTMLElement,
	preset: AnimationPreset,
	fps: number,
	onProgress?: (progress: number) => void,
	timeline?: VideoTimeline,
): Promise<Blob> {
	if (!canExportWebM()) throw new Error("this browser can't record WebM");

	const persp = stage.querySelector<HTMLElement>(".recast-shot-persp");
	const tilt = stage.querySelector<HTMLElement>(".recast-shot-tilt");
	if (!persp || !tilt) throw new Error("load an image before exporting a clip");

	const saved = {
		perspective: persp.style.perspective,
		transform: tilt.style.transform,
		opacity: tilt.style.opacity,
		transition: tilt.style.transition,
	};
	tilt.style.transition = "none";

	const bitmaps: ImageBitmap[] = [];
	try {
		applyFrame(persp, tilt, preset, 0);
		const probe = await domToCanvas(stage, { scale: 2, filter: exportFilter });
		const { width, height } = outputDims(probe);

		const { total, timeAt } = sampler(preset, fps, timeline);

		// Phase 1: render every frame offline into an ImageBitmap (progress 0..0.6).
		for (let i = 0; i < total; i++) {
			const time = timeAt(i);
			applyFrame(persp, tilt, preset, time);
			const canvas = await domToCanvas(stage, { scale: 2, filter: exportFilter });
			bitmaps.push(await createImageBitmap(canvas));
			onProgress?.(((i + 1) / total) * 0.6);
		}
		// Restore the live editor before playback (capture uses its own canvas).
		persp.style.perspective = saved.perspective;
		tilt.style.transform = saved.transform;
		tilt.style.opacity = saved.opacity;
		tilt.style.transition = saved.transition;

		const out = document.createElement("canvas");
		out.width = width;
		out.height = height;
		const octx = out.getContext("2d", { alpha: false });
		if (!octx) throw new Error("could not create a drawing context");

		// `captureStream(0)` is manual frames: one push per drawn bitmap, paced to real time.
		const stream = out.captureStream(0);
		const track = stream.getVideoTracks()[0] as CanvasCaptureMediaStreamTrack;
		const chunks: Blob[] = [];
		const rec = new MediaRecorder(stream, { mimeType: webmMime(), videoBitsPerSecond: 8_000_000 });
		rec.ondataavailable = (e) => {
			if (e.data.size) chunks.push(e.data);
		};
		const stopped = new Promise<void>((res) => {
			rec.onstop = () => res();
		});

		rec.start();
		const frameMs = 1000 / fps;
		for (let i = 0; i < bitmaps.length; i++) {
			octx.drawImage(bitmaps[i], 0, 0, width, height);
			track.requestFrame();
			onProgress?.(0.6 + ((i + 1) / bitmaps.length) * 0.4);
			await sleep(frameMs);
		}
		rec.stop();
		await stopped;
		track.stop();
		return new Blob(chunks, { type: "video/webm" });
	} finally {
		bitmaps.forEach((b) => b.close());
		persp.style.perspective = saved.perspective;
		tilt.style.transform = saved.transform;
		tilt.style.opacity = saved.opacity;
		tilt.style.transition = saved.transition;
	}
}
