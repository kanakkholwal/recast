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
import { ALL_FORMATS, Input, UrlSource, type VideoSample, VideoSampleSink } from 'mediabunny';
import { textureRingFrames } from '../cache/frame-budget';

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

/** Carries a classified code out of `init` so the caller can tell an
 *  undecodable codec from a corrupt file. */
class WorkerError extends Error {
	constructor(
		readonly code: MediabunnyErrorCode,
		message: string,
	) {
		super(message);
	}
}

/**
 * `durationSec`/`fps` are hints from the host, which already has authoritative
 * ffprobe metadata. Supplying them skips two container walks that are O(file) on
 * a fragmented MP4 — see `init`.
 */
type InitMessage = { type: 'init'; url: string; durationSec?: number; fps?: number };
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
	/** Transferred decode surface — the consumer OWNS it and must close it.
	 *  Sent straight through rather than via a canvas: routing 4K frames through
	 *  an OffscreenCanvas cost two full-frame allocations each. */
	frame: VideoFrame;
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
let sink: VideoSampleSink | null = null;
let disposed = false;

/**
 * How far ahead of the playhead to decode before parking. Derived from
 * `frameBudget().decodeAhead` at init: decoding further ahead than the cache can
 * hold just evicts those frames on arrival, and at 4K that churn (two full-frame
 * allocations each) was enough to take the renderer down. A fixed 0.75s meant 45
 * frames in flight against a 4-frame cache.
 */
let lookaheadSec = 0.1;

// Dev-only run tracing. Bounded to run start/park/end — never per frame.
const DIAG = ((): boolean => {
	try {
		return Boolean((import.meta as { env?: { DEV?: boolean } }).env?.DEV);
	} catch {
		return false;
	}
})();

/** Monotonic id; a new run supersedes the old one without an abort race. */
let runId = 0;
/** The live decode generator, so a supersede can tear its decoder down at once. */
let activeSamples: AsyncGenerator<VideoSample, void, unknown> | null = null;
let playheadSec = 0;
/** Newest timestamp the live run has posted; NaN when no run is streaming. */
let deliveredSec = Number.NaN;
/** Resolves when the playhead advances, waking a run parked on backpressure. */
let playheadWaiters: Array<() => void> = [];

/**
 * Whether the live run already covers `targetSec` — it has decoded past it, or
 * will within its lookahead. Restarting for such a target throws away a warm
 * decoder to re-decode frames the consumer was about to receive anyway.
 *
 * Only forward targets qualify: anything behind the playhead may already have
 * been evicted from the consumer's cache, so that needs a real seek.
 */
function runCovers(targetSec: number): boolean {
	if (Number.isNaN(deliveredSec) || disposed) return false;
	return targetSec >= playheadSec && targetSec <= deliveredSec + lookaheadSec;
}

function notifyPlayhead(): void {
	const waiters = playheadWaiters;
	playheadWaiters = [];
	for (const w of waiters) w();
}

function awaitPlayhead(): Promise<void> {
	return new Promise((resolve) => playheadWaiters.push(resolve));
}

async function init(url: string, hints: { durationSec?: number; fps?: number } = {}): Promise<void> {
	disposed = false;
	// Per-step timing: a slow open used to surface only as a 30s timeout with no
	// indication of which container call was walking the file.
	let stepAt = performance.now();
	const step = (label: string) => {
		if (!DIAG) return;
		const now = performance.now();
		console.log(`[mb-worker] init ${label} ${(now - stepAt).toFixed(0)}ms`);
		stepAt = now;
	};
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
		step('canRead');
		const track = await input.getPrimaryVideoTrack();
		if (!track) throw new Error('No video track in the input.');
		step('getPrimaryVideoTrack');
		// Parsing the container proves nothing about decodability — HEVC on a
		// Windows box without the codec extension parses fine and then throws on
		// the first decode, seconds later, with the picture already "ready".
		if (!(await track.canDecode())) {
			const codec = await track.getCodec();
			throw new WorkerError('unsupported', `This system can't decode ${codec ?? 'this'} video.`);
		}
		step('canDecode');
		// Nothing reads this duration — it exists only to fill the `ready`
		// payload — yet `computeDuration()` walks every fragment of a fragmented
		// MP4. On a 600MB 4K recording that alone blew the 30s init timeout.
		// Trust the host's ffprobe value when it has one.
		const durationSec = hints.durationSec ?? (await input.computeDuration());
		step('computeDuration');
		// `codedWidth` is the sync deprecated getter (returns 0 until
		// metadata loads); prefer the async variant for the ready payload.
		const width = await track.getCodedWidth();
		const height = await track.getCodedHeight();
		step('codedDimensions');
		// Samples, not canvases: the frames go straight to the consumer, so no
		// per-frame canvas allocation and no canvas→VideoFrame copy.
		sink = new VideoSampleSink(track);
		// Real rate, not a hardcoded 30: the source derives each frame's
		// duration from it, and telemetry cohorts on it.
		let fps = 30;
		try {
			// Same story as duration: sampling packets means reading them, and the
			// host already knows the real rate from ffprobe.
			const hinted = hints.fps && Number.isFinite(hints.fps) && hints.fps > 0;
			const stats = hinted ? null : await track.computePacketStats(120);
			if (hinted) {
				fps = hints.fps as number;
			} else if (stats?.averagePacketRate && Number.isFinite(stats.averagePacketRate)) {
				fps = stats.averagePacketRate;
			}
		} catch {
			/* keep the default */
		}
		// Decode-ahead is a frame count, not a duration. The consumer uploads
		// each frame to its own texture and releases it immediately, so the
		// bound is its texture ring — leave two slots of headroom so a frame
		// isn't overwritten before the playhead reaches it.
		const ahead = Math.max(2, textureRingFrames(width, height) - 2);
		lookaheadSec = ahead / Math.max(1, fps);
		step('packetStats');
		post({ type: 'ready', width, height, durationSec, fps });
	} catch (err) {
		post({
			type: 'error',
			code: err instanceof WorkerError ? err.code : 'bad-input',
			message: err instanceof Error ? err.message : String(err),
		});
		throw err;
	}
}

/**
 * Decode forward from `startSec`, posting frames in presentation order until
 * superseded, disposed, or the source ends. Parks while more than
 * `lookaheadSec` ahead of the playhead so a long clip can't decode itself
 * into memory.
 */
async function runFrom(seq: number, startSec: number): Promise<void> {
	if (!sink) {
		post({ type: 'error', code: 'worker-died', message: 'Sink not initialized.' });
		return;
	}
	const myRun = ++runId;
	playheadSec = startSec;
	deliveredSec = Number.NaN;
	let sent = 0;
	let parked = false;
	if (DIAG) console.log(`[mb-worker] run ${seq} from ${startSec.toFixed(3)}s`);
	// Kill the previous run's decoder NOW. A superseded run is blocked in
	// `for await` until its first sample arrives, so it cannot notice it has
	// been replaced — during a scrub that stacks up one live decoder per
	// pointer move and exhausts the pool.
	const previous = activeSamples;
	activeSamples = null;
	if (previous) await previous.return(undefined).catch(() => {});
	if (myRun !== runId || disposed) return;
	const samples = sink.samples(startSec);
	activeSamples = samples;
	try {
		for await (const sample of samples) {
			if (myRun !== runId || disposed) {
				sample.close();
				break;
			}
			// The sample keeps its own frame, so ours must be closed separately —
			// transferring it hands that responsibility to the consumer.
			const frame = sample.toVideoFrame();
			const timestamp = sample.timestamp;
			const width = frame.codedWidth;
			const height = frame.codedHeight;
			sample.close();
			// Post the REAL presentation timestamp, not the requested one: the
			// cache keys on it, and the reader looks up by nearest-at-or-before.
			post({ type: 'frame', seq, originalSec: timestamp, frame, width, height }, [frame]);
			sent++;
			deliveredSec = timestamp;
			while (myRun === runId && !disposed && timestamp > playheadSec + lookaheadSec) {
				if (DIAG && !parked) {
					parked = true;
					console.log(
						`[mb-worker] run ${seq} parked at ${timestamp.toFixed(3)}s ` +
							`(playhead ${playheadSec.toFixed(3)}s, lookahead ${lookaheadSec.toFixed(3)}s)`,
					);
				}
				await awaitPlayhead();
			}
			if (DIAG && parked) {
				parked = false;
				console.log(`[mb-worker] run ${seq} resumed (playhead ${playheadSec.toFixed(3)}s)`);
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
		if (DIAG) {
			const why = disposed ? 'disposed' : myRun !== runId ? 'superseded' : 'end-of-stream';
			console.log(`[mb-worker] run ${seq} ended after ${sent} frames (${why})`);
		}
		// Stop absorbing seeks into a run that is no longer streaming, or a
		// target inside its old window would be silently dropped.
		if (myRun === runId) deliveredSec = Number.NaN;
		if (activeSamples === samples) activeSamples = null;
		// Release the generator's decoder resources when superseded mid-stream.
		await samples.return(undefined).catch(() => {});
	}
}

/**
 * Decode one frame at `originalSec` without disturbing the active run, so the
 * post-cut frame is warm before the playhead crosses. Skipped when a prefetch
 * for the same target is already in flight or already delivered.
 */
let prefetchedSec = Number.NaN;
let prefetchInFlight = false;
/** Latest target asked for while busy; dropping it loses the warm GOP. */
let prefetchPending: { seq: number; originalSec: number } | null = null;

async function prefetch(seq: number, originalSec: number): Promise<void> {
	if (!sink || disposed) return;
	if (prefetchedSec === originalSec) return;
	if (prefetchInFlight) {
		prefetchPending = { seq, originalSec };
		return;
	}
	prefetchInFlight = true;
	try {
		const sample = await sink.getSample(originalSec);
		if (!sample) return;
		if (disposed) {
			sample.close();
			return;
		}
		prefetchedSec = originalSec;
		const frame = sample.toVideoFrame();
		const timestamp = sample.timestamp;
		const width = frame.codedWidth;
		const height = frame.codedHeight;
		sample.close();
		post({ type: 'frame', seq, originalSec: timestamp, frame, width, height }, [frame]);
	} catch {
		/* prefetch is best-effort */
	} finally {
		prefetchInFlight = false;
		const next = prefetchPending;
		prefetchPending = null;
		if (next && !disposed) void prefetch(next.seq, next.originalSec);
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
			void init(msg.url, { durationSec: msg.durationSec, fps: msg.fps }).catch((err) => {
				// init() already posts an error message; just log here for the
				// developer console and bail.
				console.error('[mb-worker] init failed:', err);
			});
			return;
		case 'seek':
			// A drag issues near-identical targets back to back. If the live run
			// is already streaming through this point, moving its playhead is
			// enough — superseding would kill a warm decoder and hold the
			// picture on the last frame for the whole restart.
			if (runCovers(msg.originalSec)) {
				playheadSec = msg.originalSec;
				notifyPlayhead();
				return;
			}
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
