/**
 * The conversion handlers — one per tool op, registered into `handlers` which
 * the worker dispatches against. Container/encode ops (trim, mute, resize,
 * compress, transcode, extract-audio) go through MediaBunny's `Conversion`;
 * frame/audio ops (gif, mp3, frames) read samples via sinks and encode with the
 * small encoders. Each handler reports progress, honours cancellation, and
 * maps failures to a `ConvertError` code.
 *
 * This module is the ONLY consumer of `mediabunny` types inside the package
 * (other than `input.ts` and `conversion.ts`); it does not import any
 * consumer-facing surface so the public API stays tree-shakable.
 */

import {
	AudioBufferSink,
	type AudioCodec,
	CanvasSink,
	Mp4OutputFormat,
	type VideoCodec,
	WavOutputFormat,
} from "mediabunny";
import { inputContainerKind, outputFormatFor, runConversion, withExtension } from "./conversion";
import { createGifWriter, encodeMp3, zipFiles } from "./encoders";
import { openInput } from "./input";
import { ConvertError, type ConvertHandler, type ConvertJob, type HandlerResult } from "./protocol";

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));

function result(
	data: Uint8Array | ArrayBuffer,
	name: string,
	ext: string,
	mime: string,
): HandlerResult {
	return {
		blob: new Blob([data as BlobPart], { type: mime }),
		filename: withExtension(name, ext),
		mime,
	};
}

const videoMime = (kind: "mp4" | "webm") => (kind === "webm" ? "video/webm" : "video/mp4");
const cancelled = () => new ConvertError("cancelled", "Cancelled.");

//  Tier A / C: MediaBunny Conversion

const trim: ConvertHandler = async (job, ctx) => {
	const input = await openInput(job.file);
	try {
		const kind = await inputContainerKind(input);
		const bytes = await runConversion(
			input,
			{
				outputFormat: outputFormatFor(kind),
				trim: { start: job.options.startSec, end: job.options.endSec },
			},
			ctx,
		);
		return result(bytes, job.file.name, kind, videoMime(kind));
	} finally {
		input.dispose();
	}
};

const mute: ConvertHandler = async (job, ctx) => {
	const input = await openInput(job.file);
	try {
		const kind = await inputContainerKind(input);
		const bytes = await runConversion(
			input,
			{ outputFormat: outputFormatFor(kind), audio: { discard: true } },
			ctx,
		);
		return result(bytes, job.file.name, kind, videoMime(kind));
	} finally {
		input.dispose();
	}
};

const extractAudio: ConvertHandler = async (job, ctx) => {
	const fmt = job.options.audioFormat ?? "m4a";
	if (fmt === "mp3") return audioToMp3(job, ctx);
	const input = await openInput(job.file);
	try {
		if (!(await input.getPrimaryAudioTrack())) {
			throw new ConvertError("bad-input", "This video has no audio track.");
		}
		if (fmt === "wav") {
			const bytes = await runConversion(
				input,
				{
					outputFormat: new WavOutputFormat(),
					video: { discard: true },
					audio: { codec: "pcm-s16" },
				},
				ctx,
			);
			return result(bytes, job.file.name, "wav", "audio/wav");
		}
		// m4a: copy the AAC track when possible (no re-encode).
		const bytes = await runConversion(
			input,
			{ outputFormat: new Mp4OutputFormat(), video: { discard: true } },
			ctx,
		);
		return result(bytes, job.file.name, "m4a", "audio/mp4");
	} finally {
		input.dispose();
	}
};

const transcode: ConvertHandler = async (job, ctx) => {
	const container = job.options.container ?? "mp4";
	const defaults = container === "webm" ? { v: "vp9", a: "opus" } : { v: "avc", a: "aac" };
	const input = await openInput(job.file);
	try {
		const bytes = await runConversion(
			input,
			{
				outputFormat: outputFormatFor(container),
				video: { codec: (job.options.videoCodec ?? defaults.v) as VideoCodec },
				audio: { codec: (job.options.audioCodec ?? defaults.a) as AudioCodec },
			},
			ctx,
		);
		return result(bytes, job.file.name, container, videoMime(container));
	} finally {
		input.dispose();
	}
};

const compress: ConvertHandler = async (job, ctx) => {
	const input = await openInput(job.file);
	try {
		const kind = await inputContainerKind(input);
		const bytes = await runConversion(
			input,
			{
				outputFormat: outputFormatFor(kind),
				video: { bitrate: job.options.videoBitrate ?? 1_500_000, forceTranscode: true },
			},
			ctx,
		);
		return result(bytes, job.file.name, kind, videoMime(kind));
	} finally {
		input.dispose();
	}
};

const resize: ConvertHandler = async (job, ctx) => {
	const input = await openInput(job.file);
	try {
		const kind = await inputContainerKind(input);
		const bytes = await runConversion(
			input,
			{
				outputFormat: outputFormatFor(kind),
				video: { width: job.options.width, height: job.options.height, fit: "contain" },
			},
			ctx,
		);
		return result(bytes, job.file.name, kind, videoMime(kind));
	} finally {
		input.dispose();
	}
};

//  Tier B: sinks + small encoders

const videoToGif: ConvertHandler = async (job, ctx) => {
	const fps = clamp(job.options.fps ?? 12, 1, 30);
	const width = clamp(job.options.width ?? 640, 16, 1920);
	const colors = clamp(job.options.gifColors ?? 256, 2, 256);
	const input = await openInput(job.file);
	try {
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new ConvertError("bad-input", "No video track found.");
		const duration = await input.computeDuration();
		const frameCount = Math.max(1, Math.floor(duration * fps));
		// Only a width is set, so height follows the source aspect; resolution is the lever, since a GIF scaled up from 480px reads as pixelated.
		const sink = new CanvasSink(track, { width, fit: "contain" });
		const gif = createGifWriter({ maxColors: colors, dither: job.options.gifDither !== false });
		const delayMs = 1000 / fps;

		const timestamps = (function* () {
			for (let k = 0; k < frameCount; k++) yield k / fps;
		})();

		let done = 0;
		for await (const wrapped of sink.canvasesAtTimestamps(timestamps)) {
			if (ctx.signal.aborted) throw cancelled();
			done++;
			if (!wrapped) continue;
			const { canvas } = wrapped;
			gif.addFrame(canvasRgba(canvas), canvas.width, canvas.height, delayMs);
			ctx.onProgress(done / frameCount);
		}
		return result(gif.finish(), job.file.name, "gif", "image/gif");
	} finally {
		input.dispose();
	}
};

const audioToMp3: ConvertHandler = async (job, ctx) => {
	const input = await openInput(job.file);
	try {
		const track = await input.getPrimaryAudioTrack();
		if (!track) throw new ConvertError("bad-input", "This file has no audio.");
		const duration = await input.computeDuration();
		const sink = new AudioBufferSink(track);
		const leftParts: Float32Array[] = [];
		const rightParts: Float32Array[] = [];
		let sampleRate = 48_000;
		let stereo = false;

		for await (const w of sink.buffers()) {
			if (ctx.signal.aborted) throw cancelled();
			const ab = w.buffer;
			sampleRate = ab.sampleRate;
			stereo = ab.numberOfChannels >= 2;
			leftParts.push(ab.getChannelData(0).slice());
			rightParts.push((stereo ? ab.getChannelData(1) : ab.getChannelData(0)).slice());
			if (duration > 0) ctx.onProgress(clamp((w.timestamp + w.duration) / duration, 0, 1));
		}
		const left = concatFloat(leftParts);
		const channels = stereo ? [left, concatFloat(rightParts)] : [left];
		const bytes = encodeMp3(channels, sampleRate, 192);
		return result(bytes, job.file.name, "mp3", "audio/mpeg");
	} finally {
		input.dispose();
	}
};

const extractFrames: ConvertHandler = async (job, ctx) => {
	const count = clamp(job.options.frameCount ?? 10, 1, 50);
	const fmt = job.options.imageFormat ?? "png";
	const ext = fmt === "jpeg" ? "jpg" : "png";
	const input = await openInput(job.file);
	try {
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new ConvertError("bad-input", "No video track found.");
		const duration = await input.computeDuration();
		const width = clamp(job.options.width ?? 1280, 16, 3840);
		const sink = new CanvasSink(track, { width, fit: "contain" });

		const timestamps: number[] = [];
		for (let k = 0; k < count; k++) timestamps.push((duration * (k + 0.5)) / count);

		const files: Record<string, Uint8Array> = {};
		let i = 0;
		for await (const w of sink.canvasesAtTimestamps(timestamps)) {
			if (ctx.signal.aborted) throw cancelled();
			i++;
			if (!w) continue;
			const blob = await canvasToBlob(w.canvas, fmt);
			files[`frame-${String(i).padStart(3, "0")}.${ext}`] = new Uint8Array(
				await blob.arrayBuffer(),
			);
			ctx.onProgress(i / count);
		}
		return result(zipFiles(files), job.file.name, "zip", "application/zip");
	} finally {
		input.dispose();
	}
};

//  canvas / buffer helpers --

function canvasRgba(canvas: HTMLCanvasElement | OffscreenCanvas): Uint8ClampedArray {
	const ctx2d = canvas.getContext("2d") as
		| CanvasRenderingContext2D
		| OffscreenCanvasRenderingContext2D
		| null;
	if (!ctx2d) throw new ConvertError("internal", "Couldn't read frame pixels.");
	return ctx2d.getImageData(0, 0, canvas.width, canvas.height).data;
}

function canvasToBlob(
	canvas: HTMLCanvasElement | OffscreenCanvas,
	fmt: "png" | "jpeg",
): Promise<Blob> {
	const type = fmt === "jpeg" ? "image/jpeg" : "image/png";
	if ("convertToBlob" in canvas) return canvas.convertToBlob({ type });
	return new Promise<Blob>((res, rej) =>
		canvas.toBlob((b) => (b ? res(b) : rej(new Error("toBlob failed"))), type),
	);
}

function concatFloat(parts: Float32Array[]): Float32Array {
	let total = 0;
	for (const p of parts) total += p.length;
	const out = new Float32Array(total);
	let off = 0;
	for (const p of parts) {
		out.set(p, off);
		off += p.length;
	}
	return out;
}

//  registry --

export const handlers: Partial<Record<ConvertJob["op"], ConvertHandler>> = {
	trim,
	mute,
	"extract-audio": extractAudio,
	"video-to-gif": videoToGif,
	"audio-to-mp3": audioToMp3,
	"extract-frames": extractFrames,
	transcode,
	compress,
	resize,
};
