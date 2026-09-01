/// <reference lib="webworker" />
/**
 * Filmstrip decode worker: random-access thumbnail extraction for the
 * editor's clip bar, off the main thread. Migrated from the legacy
 * mp4box + WebCodecs pipeline to MediaBunny's `Input` + `CanvasSink` so the
 * project no longer depends on `mp4box`.
 *
 * The worker holds one `Input` and one `CanvasSink`, and drains every decode
 * through a single `drain()` loop — `getCanvas` builds a fresh `VideoDecoder`
 * per call, so overlapping drains would mean one live hardware decoder per
 * in-flight message. Newest batch drains first (a scroll shouldn't wait behind
 * tiles that already left the viewport); the `storyboard` sprite for hover-scrub
 * runs only once no tiles are queued.
 *
 * Why this beats the legacy pipeline:
 *   - No mp4box: MediaBunny's `Input` parses the file (mp4/mov/webm).
 *   - No hand-rolled WebCodecs wiring: `CanvasSink` is the decode
 *     primitive, the worker just iterates `screenshotAtTimestamps`.
 *   - Concurrency model: one input + one sink, but the sink's `poolSize`
 *     lets MediaBunny do its own decode-ahead, so a flurry of
 *     `previewAt` calls in the same frame is batched internally.
 *
 * Frame ownership: canvases returned from `getCanvas` are owned by the
 * worker until they're transferred. We copy to a fresh `OffscreenCanvas`
 * sized for the requested thumbnail, blit the source, then JPEG-encode
 * and transfer the blob back to the main thread.
 */

import type { MediaRef } from "@recast/media";
import { ALL_FORMATS, CanvasSink, Input, mediaRefSource } from "@recast/media/mediabunny";
// One definition, shared with the provider: the redeclared copies had already drifted.
import type { FromFilmstripWorker, ToFilmstripWorker } from "./filmstrip-protocol";

const ctx = self as unknown as DedicatedWorkerGlobalScope;

function post(msg: FromFilmstripWorker, transfer: Transferable[] = []): void {
	ctx.postMessage(msg, transfer);
}

let input: Input | null = null;
let sink: CanvasSink | null = null;
let tileHeightPx = 2;
let videoWidth = 0;
let videoHeight = 0;
let videoDurationSec = 0;
let disposed = false;

async function init(src: MediaRef, hPx: number, durationSec?: number): Promise<void> {
	tileHeightPx = hPx;
	// Both ref kinds read lazily; a Blob materialized from the whole file is what pinned ~600MB per 4K session.
	input = new Input({
		source: mediaRefSource(src),
		formats: ALL_FORMATS,
	});
	const track = await input.getPrimaryVideoTrack();
	if (!track) throw new Error("Filmstrip: no video track in input.");
	// Trust the caller's ffprobe duration: computeDuration() walks every fragment, which is many range reads.
	videoDurationSec =
		durationSec && Number.isFinite(durationSec) ? durationSec : await input.computeDuration();
	const w = await track.getCodedWidth();
	const h = await track.getCodedHeight();
	videoWidth = w ?? 0;
	videoHeight = h ?? 0;
	// Fit the tile into the requested height, keep aspect.
	const tileWidth = Math.max(
		2,
		Math.round((tileHeightPx * (videoWidth || 1)) / (videoHeight || 1)),
	);
	sink = new CanvasSink(track, { width: tileWidth, fit: "contain" });
	// TEMP diagnostic: confirms the worker parsed the source and its dimensions/duration.
	console.info("[filmstrip] ready", { videoWidth, videoHeight, videoDurationSec, tileWidth });
	post({ type: "ready" });
}

type DecodeRequest = { id: number; originalSec: number };

/** Ceiling on queued-but-undecoded tiles. A fast scroll can request faster than
 *  the decoder drains; past this the oldest are furthest from the viewport and
 *  not worth the decode. */
const MAX_PENDING = 96;

let pending: DecodeRequest[] = [];
let storyboardQueued = false;
let draining = false;

function enqueueDecode(requests: readonly DecodeRequest[]): void {
	// Newest batch first: it reflects where the user is now, so a scroll doesn't wait on tiles already off-screen.
	pending = [...requests, ...pending];
	if (pending.length > MAX_PENDING) {
		for (const dropped of pending.splice(MAX_PENDING)) {
			post({ type: "drop", id: dropped.id });
		}
	}
	void drain();
}

async function decodeOne(req: DecodeRequest): Promise<void> {
	if (!sink) return;
	try {
		let wrapped = await sink.getCanvas(req.originalSec);
		// Recorded files often report a duration a hair past the last frame, so a tile sampled near the end decodes to nothing; step just inside and retry so short clips (where that tile is a big fraction of the strip) don't go blank.
		if (!wrapped && !disposed && req.originalSec > 0.05) {
			wrapped = await sink.getCanvas(req.originalSec - 0.05);
		}
		if (disposed) return;
		if (!wrapped) {
			// Answered, not dropped: a silent return leaves the id in the provider's in-flight set, and that cache key is never requested again for the session.
			post({
				type: "error",
				id: req.id,
				message: `no frame at ${req.originalSec.toFixed(2)}s`,
			});
			return;
		}
		const src = wrapped.canvas as OffscreenCanvas;
		const blob = await canvasToJpeg(src);
		if (disposed) return;
		// A Blob is structured-cloneable but NOT transferable; listing it throws and loses the whole tile.
		post({ type: "tile", id: req.id, blob, width: src.width, height: src.height });
	} catch (err) {
		// Carry the request id so the provider clears it from in-flight, or the tile wedges and the maps grow.
		post({
			type: "error",
			id: req.id,
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

/**
 * The ONLY place `sink.getCanvas` is driven. MediaBunny builds a fresh
 * `VideoDecoder` per `getCanvas` call, so overlapping drains meant one live
 * hardware decoder per in-flight message — tens of them during a scroll, each
 * holding its own surface pool. The `draining` latch keeps that at exactly one.
 */
async function drain(): Promise<void> {
	if (draining) return;
	draining = true;
	try {
		while (!disposed && sink) {
			const req = pending.shift();
			if (req) {
				await decodeOne(req);
				continue;
			}
			// Visible tiles always win; the storyboard is hover-scrub polish and runs only once the strip is full.
			if (storyboardQueued) {
				storyboardQueued = false;
				try {
					await buildStoryboard();
				} catch (err) {
					// A throw here must not abandon queued tiles: they would never be answered and would stay in-flight forever.
					post({
						type: "error",
						message: err instanceof Error ? err.message : String(err),
					});
				}
				continue;
			}
			break;
		}
	} finally {
		draining = false;
	}
}

async function canvasToJpeg(src: OffscreenCanvas): Promise<Blob> {
	// Re-encode at native size; 0.82 is the thumbnail sweet spot and the editor reads no EXIF.
	const blob = await src.convertToBlob({ type: "image/jpeg", quality: 0.82 });
	return blob;
}

async function buildStoryboard(): Promise<void> {
	if (!input || !sink || disposed) return;
	const cols = 8;
	const rows = 4;
	const cellW = Math.max(16, Math.round((tileHeightPx * (videoWidth || 1)) / (videoHeight || 1)));
	const cellH = tileHeightPx;
	const totalW = cellW * cols;
	const totalH = cellH * rows;
	const count = cols * rows;
	try {
		const sprite = new OffscreenCanvas(totalW, totalH);
		const ctx2d = sprite.getContext("2d", { alpha: false });
		if (!ctx2d) {
			post({ type: "error", message: "Filmstrip: cannot acquire 2D context for storyboard." });
			return;
		}
		ctx2d.fillStyle = "#000";
		ctx2d.fillRect(0, 0, totalW, totalH);
		const timestamps: number[] = [];
		for (let i = 0; i < count; i++) {
			timestamps.push(((i + 0.5) / count) * videoDurationSec);
		}
		for (let i = 0; i < timestamps.length; i++) {
			if (disposed) return;
			const wrapped = await sink.getCanvas(timestamps[i] ?? 0);
			if (!wrapped) continue;
			const src = wrapped.canvas as OffscreenCanvas;
			const col = i % cols;
			const row = Math.floor(i / cols);
			ctx2d.drawImage(src, col * cellW, row * cellH, cellW, cellH);
		}
		const blob = await sprite.convertToBlob({ type: "image/jpeg", quality: 0.85 });
		post({
			type: "storyboard",
			blob,
			cols,
			rows,
			cellW,
			cellH,
			count,
			durationSec: videoDurationSec,
		});
	} catch (err) {
		post({
			type: "storyboard-error",
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

function dispose(): void {
	disposed = true;
	pending = [];
	storyboardQueued = false;
	if (input) {
		try {
			input.dispose();
		} catch {
			/* ignore */
		}
		input = null;
	}
	sink = null;
}

/** Install this worker's RPC on its global scope. Called by the host app's
 *  entry module — this package never spawns a worker itself. */
export function startFilmstripWorker(): void {
	ctx.onmessage = (e: MessageEvent<ToFilmstripWorker>) => {
		const msg = e.data;
		switch (msg.type) {
			case "init":
				void init(msg.src, msg.tileHeightPx, msg.durationSec).catch((err) => {
					post({
						type: "error",
						message: err instanceof Error ? err.message : String(err),
					});
				});
				return;
			case "decode":
				enqueueDecode(msg.requests);
				return;
			case "storyboard":
				storyboardQueued = true;
				void drain();
				return;
			case "dispose":
				dispose();
				return;
		}
	};
}
