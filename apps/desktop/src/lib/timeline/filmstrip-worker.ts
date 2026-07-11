/// <reference lib="webworker" />
/**
 * Filmstrip decode worker: random-access thumbnail extraction, off the main
 * thread. Separate from the preview decoder (webcodecs-worker.ts) because the
 * access pattern is the opposite: sparse seeks that decode one frame and stop,
 * not sequential decode-ahead. Sharing the preview's cache would make the two
 * fight; here each thumbnail is decoded, downscaled, and the frame released.
 *
 * Decode requests are drained from a FIFO queue one batch at a time; the single
 * decoder can't be driven concurrently. Within a batch, requests are grouped by
 * GOP keyframe so one keyframe decode serves every tile in it, then matching
 * frames are drawn onto a small OffscreenCanvas and emitted as JPEG blobs.
 * Whole-file ingestion only; the provider routes huge/progressive files to the
 * Rust strip fallback instead. The provider only enqueues on-screen tiles, so
 * the queue stays bounded by what virtualization asks for.
 */

import {
	buildKeyframes,
	buildPresOrder,
	type ChunkMeta,
	keyframeAtOrBefore,
	sampleAtOrBefore,
} from "../playback/frame-index";
import { demuxWholeFile } from "../playback/mp4-demux";
import type { FromFilmstripWorker, ToFilmstripWorker } from "./filmstrip-protocol";
import { planStoryboard, storyboardSampleSec } from "./storyboard";

const ctx = self as unknown as DedicatedWorkerGlobalScope;

let chunks: ChunkMeta[] = [];
let keyframes: number[] = [];
let presOrder: number[] = [];
let decoder: VideoDecoder | null = null;
let config: VideoDecoderConfig | null = null;
let canvas: OffscreenCanvas | null = null;
let canvasCtx: OffscreenCanvasRenderingContext2D | null = null;
let thumbW = 2;
let thumbH = 2;
let videoDurationSec = 0;
let disposed = false;

/** Pending tile requests; drained one batch at a time by `drain`. */
let queue: Array<{ id: number; originalSec: number }> = [];
/** Set when the storyboard sprite has been requested; built once the tile
 *  queue is idle so on-screen filmstrip tiles aren't starved by it. */
let storyboardPending = false;
let draining = false;
/** Frames decoded for the GOP currently in flight (the output callback fills it). */
let gopFrames: VideoFrame[] = [];

function post(msg: FromFilmstripWorker, transfer: Transferable[] = []): void {
	ctx.postMessage(msg, transfer);
}

function makeDecoder(): VideoDecoder {
	return new VideoDecoder({
		output: (frame) => {
			if (disposed) {
				frame.close();
				return;
			}
			gopFrames.push(frame);
		},
		error: (e) => post({ type: "error", message: String(e) }),
	});
}

async function init(buffer: ArrayBuffer, tileHeightPx: number): Promise<void> {
	try {
		const res = await demuxWholeFile(buffer);
		chunks = res.chunks;
		keyframes = buildKeyframes(chunks);
		presOrder = buildPresOrder(chunks);
		config = res.config;
		const aspect = res.height > 0 ? res.width / res.height : 16 / 9;
		thumbH = Math.max(2, Math.round(tileHeightPx));
		thumbW = Math.max(2, Math.round(thumbH * aspect));
		videoDurationSec = res.durationSec;
		canvas = new OffscreenCanvas(thumbW, thumbH);
		canvasCtx = canvas.getContext("2d");
		decoder = makeDecoder();
		decoder.configure(config);
		post({
			type: "ready",
			width: res.width,
			height: res.height,
			durationSec: res.durationSec,
			fps: res.fps,
		});
	} catch (err) {
		post({
			type: "error",
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

interface TileItem {
	id: number;
	sample: number;
	targetTs: number;
	kf: number;
}

function enqueue(requests: Array<{ id: number; originalSec: number }>): void {
	queue.push(...requests);
	if (!draining) void drain();
}

function requestStoryboard(): void {
	storyboardPending = true;
	if (!draining) void drain();
}

/** Process queued requests one batch at a time until the queue empties, then
 *  build the storyboard if asked. Both share the single decoder, so they must
 *  run serially; on-screen tiles take priority over the storyboard. Each op is
 *  isolated in try/catch and `draining` resets in `finally`, so one decode
 *  failure can't wedge the loop and stall every later thumbnail/preview. */
async function drain(): Promise<void> {
	draining = true;
	try {
		while ((queue.length > 0 || storyboardPending) && !disposed) {
			if (queue.length > 0) {
				const batch = queue;
				queue = [];
				try {
					await decodeBatch(batch);
				} catch (err) {
					post({ type: "error", message: `tile decode: ${String(err)}` });
				}
			} else if (storyboardPending) {
				storyboardPending = false;
				try {
					await buildStoryboard();
				} catch (err) {
					post({ type: "error", message: `storyboard: ${String(err)}` });
				}
			}
		}
	} finally {
		draining = false;
	}
}

async function decodeBatch(
	requests: Array<{ id: number; originalSec: number }>,
): Promise<void> {
	if (disposed || !decoder || !config || chunks.length === 0) return;

	const items: TileItem[] = requests.map((r) => {
		const tUs = Math.max(0, Math.round(r.originalSec * 1e6));
		const sample = sampleAtOrBefore(chunks, presOrder, tUs);
		return {
			id: r.id,
			sample,
			targetTs: chunks[sample].ctsUs,
			kf: keyframeAtOrBefore(keyframes, sample),
		};
	});

	const byGop = new Map<number, TileItem[]>();
	for (const it of items) {
		const list = byGop.get(it.kf);
		if (list) list.push(it);
		else byGop.set(it.kf, [it]);
	}

	// Ascending so the decoder feeds forward through the file.
	for (const kf of [...byGop.keys()].sort((a, b) => a - b)) {
		if (disposed) return;
		await decodeGop(kf, byGop.get(kf) as TileItem[]);
	}
}

async function decodeGop(kf: number, reqs: TileItem[]): Promise<void> {
	if (!decoder || !config) return;
	const feedEnd = Math.min(
		Math.max(...reqs.map((r) => r.sample)),
		chunks.length - 1,
	);

	gopFrames = [];
	decoder.reset();
	decoder.configure(config);
	for (let i = kf; i <= feedEnd; i++) {
		const c = chunks[i];
		decoder.decode(
			new EncodedVideoChunk({
				type: c.key ? "key" : "delta",
				timestamp: c.ctsUs,
				duration: c.durUs,
				data: c.data as Uint8Array,
			}),
		);
	}
	await decoder.flush();

	const frames = gopFrames;
	gopFrames = [];
	if (disposed) {
		for (const f of frames) f.close();
		return;
	}

	for (const req of reqs) {
		const frame = pickFrame(frames, req.targetTs);
		if (frame && canvasCtx && canvas) {
			canvasCtx.drawImage(frame, 0, 0, thumbW, thumbH);
			const blob = await canvas.convertToBlob({
				type: "image/jpeg",
				quality: 0.72,
			});
			if (disposed) break;
			post({ type: "tile", id: req.id, blob });
		}
	}
	for (const f of frames) f.close();
}

/**
 * Build the storyboard sprite: decode `count` frames evenly spaced across the
 * duration and pack them into one `cols`×`rows` canvas, GOP-grouped like the
 * tile path so a keyframe decode serves every cell in its GOP. Emitted as a
 * single JPEG so hover-scrub crops a cell instead of decoding per position.
 */
async function buildStoryboard(): Promise<void> {
	if (disposed || !decoder || !config || chunks.length === 0) return;
	const duration = videoDurationSec;
	if (duration <= 0) return;

	// Grid sizing shared with the hover preview (storyboard.ts), so both agree.
	const { count, cols, rows } = planStoryboard(duration);
	const cellW = thumbW;
	const cellH = thumbH;
	const sheet = new OffscreenCanvas(cols * cellW, rows * cellH);
	const sheetCtx = sheet.getContext("2d");
	if (!sheetCtx) return;

	// One cell per evenly-spaced sample time; group by GOP like decodeBatch.
	const items = Array.from({ length: count }, (_, cell) => {
		const tUs = Math.max(
			0,
			Math.round(storyboardSampleSec(cell, count, duration) * 1e6),
		);
		const sample = sampleAtOrBefore(chunks, presOrder, tUs);
		return {
			cell,
			sample,
			targetTs: chunks[sample].ctsUs,
			kf: keyframeAtOrBefore(keyframes, sample),
		};
	});
	const byGop = new Map<number, typeof items>();
	for (const it of items) {
		const list = byGop.get(it.kf);
		if (list) list.push(it);
		else byGop.set(it.kf, [it]);
	}

	for (const kf of [...byGop.keys()].sort((a, b) => a - b)) {
		if (disposed) return;
		// On-screen tiles / hover-scrub frames jump ahead between GOPs, so the
		// preview stays responsive while the (slower) full storyboard builds.
		while (queue.length > 0 && !disposed) {
			const batch = queue;
			queue = [];
			await decodeBatch(batch);
		}
		if (disposed) return;
		const reqs = byGop.get(kf) as typeof items;
		const feedEnd = Math.min(
			Math.max(...reqs.map((r) => r.sample)),
			chunks.length - 1,
		);
		gopFrames = [];
		decoder.reset();
		decoder.configure(config);
		for (let i = kf; i <= feedEnd; i++) {
			const c = chunks[i];
			decoder.decode(
				new EncodedVideoChunk({
					type: c.key ? "key" : "delta",
					timestamp: c.ctsUs,
					duration: c.durUs,
					data: c.data as Uint8Array,
				}),
			);
		}
		await decoder.flush();
		const frames = gopFrames;
		gopFrames = [];
		if (disposed) {
			for (const f of frames) f.close();
			return;
		}
		for (const req of reqs) {
			const frame = pickFrame(frames, req.targetTs);
			if (frame) {
				const col = req.cell % cols;
				const row = Math.floor(req.cell / cols);
				sheetCtx.drawImage(frame, col * cellW, row * cellH, cellW, cellH);
			}
		}
		for (const f of frames) f.close();
	}

	if (disposed) return;
	const blob = await sheet.convertToBlob({ type: "image/jpeg", quality: 0.72 });
	if (disposed) return;
	post({
		type: "storyboard",
		blob,
		cols,
		rows,
		cellW,
		cellH,
		count,
		durationSec: duration,
	});
}

/** The decoded frame at-or-before `targetTs` (closest), or null if none. */
function pickFrame(frames: VideoFrame[], targetTs: number): VideoFrame | null {
	let best: VideoFrame | null = null;
	let bestTs = -Infinity;
	for (const f of frames) {
		if (f.timestamp <= targetTs && f.timestamp > bestTs) {
			bestTs = f.timestamp;
			best = f;
		}
	}
	// Before the first frame's cts (rare rounding), fall back to the earliest.
	if (!best && frames.length > 0) {
		best = frames.reduce((a, b) => (a.timestamp <= b.timestamp ? a : b));
	}
	return best;
}

function dispose(): void {
	if (disposed) return;
	disposed = true;
	queue = [];
	for (const f of gopFrames) f.close();
	gopFrames = [];
	try {
		if (decoder && decoder.state !== "closed") decoder.close();
	} catch {
		/* already closing */
	}
	decoder = null;
	chunks = [];
	ctx.close();
}

ctx.onmessage = (e: MessageEvent<ToFilmstripWorker>) => {
	const msg = e.data;
	switch (msg.type) {
		case "init":
			void init(msg.buffer, msg.tileHeightPx);
			break;
		case "decode":
			enqueue(msg.requests);
			break;
		case "storyboard":
			requestStoryboard();
			break;
		case "dispose":
			dispose();
			break;
	}
};
