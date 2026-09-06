/// <reference lib="webworker" />
/**
 * Filmstrip decode worker: random-access thumbnail extraction for the
 * editor's clip bar, off the main thread. Migrated from the legacy
 * mp4box + WebCodecs pipeline to MediaBunny's `Input` + `CanvasSink` so the
 * project no longer depends on `mp4box`.
 *
 * The worker holds one `Input` and one `CanvasSink`, and drains queued tiles
 * through a single `drain()` loop that batches each pass into one
 * `canvasesAtTimestamps` call — one decoder streams the span and decodes each
 * shared GOP once, instead of `getCanvas` rebuilding a decoder per tile. The
 * `draining` latch keeps exactly one decoder alive. Newest tiles drain first (a
 * scroll shouldn't wait behind tiles that already left the viewport); the
 * `storyboard` sprite for hover-scrub runs only once no tiles are queued.
 *
 * `getCanvas` remains only for the near-end retry, where a single miss is
 * cheaper to re-request than to re-batch.
 *
 * Frame ownership: canvases are owned by the worker until transferred. We copy
 * to a fresh `OffscreenCanvas` sized for the requested thumbnail, JPEG-encode,
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

/** Tiles decoded per canvasesAtTimestamps pass. Bounded so a fresh scroll (its
 *  requests prepended to `pending`) preempts after the current small batch
 *  rather than waiting on the whole queue. */
const TILE_BATCH = 12;

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

async function postTile(req: DecodeRequest, src: OffscreenCanvas): Promise<void> {
	const blob = await canvasToJpeg(src);
	if (disposed) return;
	// A Blob is structured-cloneable but NOT transferable; listing it throws and loses the whole tile.
	post({ type: "tile", id: req.id, blob, width: src.width, height: src.height });
}

// Recorded duration overshoots the last frame, so a near-end tile decodes to nothing; step just inside so short clips don't go blank.
async function decodeNearEndFallback(req: DecodeRequest): Promise<void> {
	if (!sink) return;
	try {
		const wrapped = req.originalSec > 0.05 ? await sink.getCanvas(req.originalSec - 0.05) : null;
		if (disposed) return;
		if (wrapped) {
			await postTile(req, wrapped.canvas as OffscreenCanvas);
			return;
		}
		// Answered, not dropped: a silent return leaves the id in the provider's in-flight set, never re-requested this session.
		post({ type: "error", id: req.id, message: `no frame at ${req.originalSec.toFixed(2)}s` });
	} catch (err) {
		post({ type: "error", id: req.id, message: err instanceof Error ? err.message : String(err) });
	}
}

async function flushMisses(misses: readonly DecodeRequest[]): Promise<void> {
	for (const req of misses) {
		if (disposed) return;
		await decodeNearEndFallback(req);
	}
}

// One failed pass must not wedge its tiles in the provider's in-flight set.
function failRemaining(reqs: readonly DecodeRequest[], err: unknown): void {
	const message = err instanceof Error ? err.message : String(err);
	for (const r of reqs) post({ type: "error", id: r.id, message });
}

// One decoder pass for the batch: canvasesAtTimestamps decodes each packet once, versus getCanvas spinning a fresh decoder per tile and re-decoding shared GOPs.
async function decodeBatch(reqs: DecodeRequest[]): Promise<void> {
	if (!sink || reqs.length === 0) return;
	// Ascending times keep the one decoder streaming forward; results come back in that order, so index maps to `ordered`.
	const ordered = [...reqs].sort((a, b) => a.originalSec - b.originalSec);
	const misses: DecodeRequest[] = [];
	let i = 0;
	try {
		for await (const wrapped of sink.canvasesAtTimestamps(ordered.map((r) => r.originalSec))) {
			if (disposed) return;
			const req = ordered[i++];
			if (wrapped) await postTile(req, wrapped.canvas as OffscreenCanvas);
			else misses.push(req);
		}
	} catch (err) {
		failRemaining(ordered.slice(Math.max(0, i - 1)), err);
		return;
	}
	await flushMisses(misses);
}

/**
 * Drains queued tiles through `canvasesAtTimestamps` in bounded batches. A
 * single-flight `draining` latch keeps exactly one decoder alive; batching then
 * makes that one decoder decode each GOP once for the whole span, instead of
 * `getCanvas` rebuilding a decoder and re-decoding the GOP per tile.
 */
async function drain(): Promise<void> {
	if (draining) return;
	draining = true;
	try {
		while (!disposed && sink) {
			if (pending.length > 0) {
				await decodeBatch(pending.splice(0, TILE_BATCH));
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
		// One decoder pass for the whole sprite: getCanvas per cell spun 32 fresh VideoDecoders that re-decode each GOP and contend with the preview decoder.
		let i = 0;
		for await (const wrapped of sink.canvasesAtTimestamps(timestamps)) {
			if (disposed) return;
			if (wrapped) {
				const src = wrapped.canvas as OffscreenCanvas;
				const col = i % cols;
				const row = Math.floor(i / cols);
				ctx2d.drawImage(src, col * cellW, row * cellH, cellW, cellH);
			}
			i++;
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
