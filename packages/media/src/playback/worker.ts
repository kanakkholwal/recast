/// <reference lib="webworker" />
/**
 * MediaBunny-backed video decode worker. The worker is the lifecycle owner
 * for the MediaBunny `Input` + `CanvasSink`. The main-thread side talks to
 * it via the `ToMediabunnyWorker` / `FromMediabunnyWorker` postMessage RPC.
 *
 * A `seek` starts a decode run that streams frames forward in presentation
 * order; `playhead` only releases backpressure, so steady playback never
 * restarts decode. Frames transfer back as OffscreenCanvas.
 */

// This worker now lives INSIDE `packages/media`, so it imports MediaBunny
// directly rather than bouncing through the package barrel.
import { ALL_FORMATS, CanvasSink, Input, UrlSource } from 'mediabunny';

/** Mirror of `MediaErrorCode` (REQUIREMENTS.md §2). Kept in-worker because
 *  the worker doesn't import from `@recast/media` to avoid a worker-side
 *  cycle through the package barrel. */
type MediabunnyErrorCode =
	| 'unsupported'
	| 'bad-input'
	| 'worker-died'
	| 'cancelled'
	| 'internal'
	| 'too-large';

type InitMessage = { type: 'init'; url: string };
type SeekMessage = { type: 'seek'; seq: number; originalSec: number };
/** Playhead advanced normally; feeds decode-ahead backpressure, never seeks. */
type PlayheadMessage = { type: 'playhead'; originalSec: number };
type PrefetchMessage = { type: 'prefetch'; seq: number; originalSec: number; lookaheadSec?: number };
type DisposeMessage = { type: 'dispose' };

export type ToMediabunnyWorker =
	| InitMessage
	| SeekMessage
	| PlayheadMessage
	| PrefetchMessage
	| DisposeMessage;

type ReadyMessage = {
	type: 'ready';
	width: number;
	height: number;
	durationSec: number;
	fps: number;
};

type FrameMessage = {
	type: 'frame';
	seq: number;
	/** Real presentation timestamp of this frame, seconds. The cache keys on it. */
	originalSec: number;
	/** Transferable canvas — the consumer uploads it to WebGL or converts
	 *  to a `VideoFrame` (e.g. `new VideoFrame(canvas)`). */
	canvas: OffscreenCanvas;
	width: number;
	height: number;
};

type ErrorMessage = { type: 'error'; code: MediabunnyErrorCode; message: string };

export type FromMediabunnyWorker = ReadyMessage | FrameMessage | ErrorMessage;

/** Bound by `startMediabunnyWorker`, so importing this module outside a
 *  worker (tooling, tests) doesn't touch `self` at evaluation time. */
let ctx: DedicatedWorkerGlobalScope | null = null;

function post(msg: FromMediabunnyWorker, transfer: Transferable[] = []): void {
	ctx?.postMessage(msg, transfer);
}

let input: Input | null = null;
let sink: CanvasSink | null = null;
let disposed = false;

/**
 * How far ahead of the playhead to decode before pausing. One seek per frame
 * (the old model) made every request abort the previous one, so nothing ever
 * finished once decode cost more than a frame interval.
 */
const LOOKAHEAD_SEC = 0.75;

/** Monotonic id; a new run supersedes the old one without an abort race. */
let runId = 0;
let playheadSec = 0;
/** Resolves when the playhead advances, waking a run parked on backpressure. */
let playheadWaiters: Array<() => void> = [];

function notifyPlayhead(): void {
	const waiters = playheadWaiters;
	playheadWaiters = [];
	for (const w of waiters) w();
}

function awaitPlayhead(): Promise<void> {
	return new Promise((resolve) => playheadWaiters.push(resolve));
}

async function init(url: string): Promise<void> {
	disposed = false;
	// `UrlSource` makes fetch() calls internally; for Tauri desktop the
	// asset-protocol URLs (`asset://localhost/...` and `tauri://...`) flow
	// through Tauri webview's network layer, same as the legacy
	// `WebCodecsVideoSource` does for its progressive ingestion path.
	input = new Input({
		source: new UrlSource(url),
		formats: ALL_FORMATS,
	});
	try {
		if (!(await input.canRead())) {
			throw new Error("MediaBunny couldn't read this file.");
		}
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new Error('No video track in the input.');
		const durationSec = await input.computeDuration();
		// `codedWidth` is the sync deprecated getter (returns 0 until
		// metadata loads); prefer the async variant for the ready payload.
		const width = await track.getCodedWidth();
		const height = await track.getCodedHeight();
		// NO pool. Pooled canvases are recycled round-robin, but we TRANSFER
		// each one to the main thread, which detaches it — the sink then draws
		// into a detached canvas and the run dies ~poolSize frames in.
		sink = new CanvasSink(track, { fit: 'contain' });
		// Real rate, not a hardcoded 30: the source derives each frame's
		// duration from it, and telemetry cohorts on it.
		let fps = 30;
		try {
			const stats = await track.computePacketStats(120);
			if (stats?.averagePacketRate && Number.isFinite(stats.averagePacketRate)) {
				fps = stats.averagePacketRate;
			}
		} catch {
			/* keep the default */
		}
		post({ type: 'ready', width, height, durationSec, fps });
	} catch (err) {
		post({
			type: 'error',
			code: 'bad-input',
			message: err instanceof Error ? err.message : String(err),
		});
		throw err;
	}
}

/**
 * Decode forward from `startSec`, posting frames in presentation order until
 * superseded, disposed, or the source ends. Parks while more than
 * `LOOKAHEAD_SEC` ahead of the playhead so a long clip can't decode itself
 * into memory.
 */
async function runFrom(seq: number, startSec: number): Promise<void> {
	if (!sink) {
		post({ type: 'error', code: 'worker-died', message: 'Sink not initialized.' });
		return;
	}
	const myRun = ++runId;
	playheadSec = startSec;
	const frames = sink.canvases(startSec);
	try {
		for await (const wrapped of frames) {
			if (myRun !== runId || disposed) break;
			if (!wrapped) continue;
			const canvas = wrapped.canvas as OffscreenCanvas;
			// Post the REAL presentation timestamp, not the requested one: the
			// cache keys on it, and the reader looks up by nearest-at-or-before.
			post(
				{
					type: 'frame',
					seq,
					originalSec: wrapped.timestamp,
					canvas,
					width: canvas.width,
					height: canvas.height,
				},
				[canvas],
			);
			while (
				myRun === runId &&
				!disposed &&
				wrapped.timestamp > playheadSec + LOOKAHEAD_SEC
			) {
				await awaitPlayhead();
			}
		}
	} catch (err) {
		if (myRun === runId && !disposed) {
			post({
				type: 'error',
				code: 'internal',
				message: err instanceof Error ? err.message : String(err),
			});
		}
	} finally {
		// Release the generator's decoder resources when superseded mid-stream.
		await frames.return(undefined).catch(() => {});
	}
}

/**
 * Decode one frame at `originalSec` without disturbing the active run, so the
 * post-cut frame is warm before the playhead crosses. Skipped when a prefetch
 * for the same target is already in flight or already delivered.
 */
let prefetchedSec = Number.NaN;
let prefetchInFlight = false;

async function prefetch(seq: number, originalSec: number): Promise<void> {
	if (!sink || disposed) return;
	if (prefetchInFlight || prefetchedSec === originalSec) return;
	prefetchInFlight = true;
	try {
		const wrapped = await sink.getCanvas(originalSec);
		if (!wrapped || disposed) return;
		prefetchedSec = originalSec;
		const canvas = wrapped.canvas as OffscreenCanvas;
		post(
			{
				type: 'frame',
				seq,
				originalSec: wrapped.timestamp,
				canvas,
				width: canvas.width,
				height: canvas.height,
			},
			[canvas],
		);
	} catch {
		/* prefetch is best-effort */
	} finally {
		prefetchInFlight = false;
	}
}

function dispose(): void {
	disposed = true;
	runId++;
	notifyPlayhead();
	sink = null;
	if (input) {
		input.dispose();
		input = null;
	}
}

/**
 * Install the decode RPC on this worker's global scope. Called by the host
 * app's worker entry module — the package never spawns the worker itself, so
 * the `new Worker(new URL(...))` URL always resolves against the app's root.
 */
export function startMediabunnyWorker(): void {
	ctx = self as unknown as DedicatedWorkerGlobalScope;
	ctx.onmessage = handleMessage;
}

function handleMessage(e: MessageEvent<ToMediabunnyWorker>): void {
	const msg = e.data;
	switch (msg.type) {
		case 'init':
			void init(msg.url).catch((err) => {
				// init() already posts an error message; just log here for the
				// developer console and bail.
				console.error('[mb-worker] init failed:', err);
			});
			return;
		case 'seek':
			// A jump: supersede the current run and decode from the new point.
			prefetchedSec = Number.NaN;
			// Supersede BEFORE waking. A run parked on backpressure only
			// re-checks runId once woken, so waking it first just re-parks it,
			// and it holds its VideoDecoder until some unrelated later message.
			runId++;
			notifyPlayhead();
			void runFrom(msg.seq, msg.originalSec);
			return;
		case 'playhead':
			// Steady playback: only releases backpressure, never restarts decode.
			playheadSec = msg.originalSec;
			notifyPlayhead();
			return;
		case 'prefetch':
			void prefetch(msg.seq, msg.originalSec);
			return;
		case 'dispose':
			dispose();
			return;
	}
}
