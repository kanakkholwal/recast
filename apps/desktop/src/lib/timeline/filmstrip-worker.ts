/// <reference lib="webworker" />
/**
 * Filmstrip decode worker: random-access thumbnail extraction for the
 * editor's clip bar, off the main thread. Migrated from the legacy
 * mp4box + WebCodecs pipeline to MediaBunny's `Input` + `CanvasSink` so the
 * project no longer depends on `mp4box`.
 *
 * The worker holds one `Input` and one `CanvasSink`. Decodes are FIFO;
 * a `decode` message queues timestamps, the worker drains them, and
 * replies with a JPEG blob per timestamp. A `storyboard` message builds
 * a single sprite image of all thumbnails in one frame, used for
 * instant hover-scrub.
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

// biome-ignore lint/style/noRestrictedImports: this worker composes
// MediaBunny primitives through `@recast/media` (the allowed channel —
// see the override in biome.json). Same scope rule as the other worker
// files in this package.
import { ALL_FORMATS, CanvasSink, Input, UrlSource } from '@recast/media/mediabunny';

type InitMessage = { type: 'init'; url: string; tileHeightPx: number; durationSec?: number };
type DecodeMessage = {
	type: 'decode';
	requests: Array<{ id: number; originalSec: number }>;
};
type StoryboardMessage = { type: 'storyboard' };
type DisposeMessage = { type: 'dispose' };

type ToFilmstripWorker = InitMessage | DecodeMessage | StoryboardMessage | DisposeMessage;

type ReadyMessage = { type: 'ready' };
type TileMessage = { type: 'tile'; id: number; blob: Blob; width: number; height: number };
type StoryboardResultMessage = {
	type: 'storyboard';
	blob: Blob;
	cols: number;
	rows: number;
	cellW: number;
	cellH: number;
	count: number;
	durationSec: number;
};
type ErrorMessage = { type: 'error'; message: string; id?: number };

type FromFilmstripWorker = ReadyMessage | TileMessage | StoryboardResultMessage | ErrorMessage;

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

async function init(url: string, hPx: number, durationSec?: number): Promise<void> {
	tileHeightPx = hPx;
	// UrlSource range-streams the file; it never holds the whole thing resident,
	// unlike the old BlobSource that pinned ~600MB (plus a spec-mandated Blob
	// copy) for the entire session on a 4K recording.
	input = new Input({
		source: new UrlSource(url),
		formats: ALL_FORMATS,
	});
	const track = await input.getPrimaryVideoTrack();
	if (!track) throw new Error('Filmstrip: no video track in input.');
	// Trust the caller's ffprobe duration; computeDuration() walks every fragment
	// of a fragmented MP4, which over a streamed source means many range reads.
	videoDurationSec =
		durationSec && Number.isFinite(durationSec) ? durationSec : await input.computeDuration();
	const w = await track.getCodedWidth();
	const h = await track.getCodedHeight();
	videoWidth = w ?? 0;
	videoHeight = h ?? 0;
	// Fit the tile into the requested height, keep aspect.
	const tileWidth = Math.max(2, Math.round((tileHeightPx * (videoWidth || 1)) / (videoHeight || 1)));
	sink = new CanvasSink(track, { width: tileWidth, fit: 'contain' });
	post({ type: 'ready' });
}

async function decodeRequests(requests: Array<{ id: number; originalSec: number }>): Promise<void> {
	if (!sink || disposed) return;
	// Single sink: drain in order. MediaBunny's internal pool pre-decodes
	// a few ahead, so back-to-back requests don't stall.
	for (const req of requests) {
		if (disposed) return;
		try {
			const wrapped = await sink.getCanvas(req.originalSec);
			if (!wrapped || disposed) continue;
			const src = wrapped.canvas as OffscreenCanvas;
			const blob = await canvasToJpeg(src);
			if (disposed) return;
			// A Blob is structured-cloneable but NOT transferable; listing it
			// throws and loses the whole tile.
			post({ type: 'tile', id: req.id, blob, width: src.width, height: src.height });
		} catch (err) {
			// Carry the request id so the provider clears it from in-flight;
			// without it the tile is wedged forever and the id/inflight maps grow.
			post({
				type: 'error',
				id: req.id,
				message: err instanceof Error ? err.message : String(err),
			});
		}
	}
}

async function canvasToJpeg(src: OffscreenCanvas): Promise<Blob> {
	// Re-encode at the tile's native size. Quality 0.82 is the visual sweet
	// spot for thumbnails; the editor doesn't read EXIF or any deep
	// metadata, so the lossy path is fine.
	const blob = await src.convertToBlob({ type: 'image/jpeg', quality: 0.82 });
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
	const sprite = new OffscreenCanvas(totalW, totalH);
	const ctx2d = sprite.getContext('2d', { alpha: false });
	if (!ctx2d) {
		post({ type: 'error', message: 'Filmstrip: cannot acquire 2D context for storyboard.' });
		return;
	}
	ctx2d.fillStyle = '#000';
	ctx2d.fillRect(0, 0, totalW, totalH);
	const count = cols * rows;
	const timestamps: number[] = [];
	for (let i = 0; i < count; i++) {
		timestamps.push(((i + 0.5) / count) * videoDurationSec);
	}
	try {
		for (let i = 0; i < timestamps.length; i++) {
			if (disposed) return;
			const wrapped = await sink.getCanvas(timestamps[i] ?? 0);
			if (!wrapped) continue;
			const src = wrapped.canvas as OffscreenCanvas;
			const col = i % cols;
			const row = Math.floor(i / cols);
			ctx2d.drawImage(src, col * cellW, row * cellH, cellW, cellH);
		}
		const blob = await sprite.convertToBlob({ type: 'image/jpeg', quality: 0.85 });
		post({
			type: 'storyboard',
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
			type: 'error',
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

function dispose(): void {
	disposed = true;
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

ctx.onmessage = (e: MessageEvent<ToFilmstripWorker>) => {
	const msg = e.data;
	switch (msg.type) {
		case 'init':
			void init(msg.url, msg.tileHeightPx, msg.durationSec).catch((err) => {
				post({
					type: 'error',
					message: err instanceof Error ? err.message : String(err),
				});
			});
			return;
		case 'decode':
			void decodeRequests(msg.requests);
			return;
		case 'storyboard':
			void buildStoryboard();
			return;
		case 'dispose':
			dispose();
			return;
	}
};