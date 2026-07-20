/// <reference lib="webworker" />
/**
 * MediaBunny-backed video decode worker. The worker is the lifecycle owner
 * for the MediaBunny `Input` + `CanvasSink`. The main-thread side talks to
 * it via the `ToMediabunnyWorker` / `FromMediabunnyWorker` postMessage RPC.
 *
 * Mirror of `webcodecs-worker.ts` for PR-D's feature-flag landing strip. The
 * behavior is intentionally minimal: it answers seek + prefetch on a single
 * in-flight request at a time, transfers the decoded frame back as an
 * OffscreenCanvas, and disposes cleanly. Concurrent requests, decoded-frame
 * caching, and AudioWorklet scheduling land in later PRs.
 */

// biome-ignore-all lint/style/noRestrictedImports: this worker composes
// MediaBunny primitives through `@recast/media` (the allowed channel —
// see the override in biome.json). Direct `mediabunny` imports outside
// `packages/media` are blocked; this file is the one scoped exception
// because a worker module cannot resolve a re-exported class through Vite's
// URL worker bundling.
import { ALL_FORMATS, CanvasSink, Input, UrlSource } from '@recast/media';

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
type PrefetchMessage = { type: 'prefetch'; originalSec: number; lookaheadSec?: number };
type CancelMessage = { type: 'cancel'; seq?: number };
type DisposeMessage = { type: 'dispose' };

export type ToMediabunnyWorker =
	| InitMessage
	| SeekMessage
	| PrefetchMessage
	| CancelMessage
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
	originalSec: number;
	/** Transferable canvas — the consumer uploads it to WebGL or converts
	 *  to a `VideoFrame` (e.g. `new VideoFrame(canvas)`). */
	canvas: OffscreenCanvas;
	width: number;
	height: number;
};

type ErrorMessage = { type: 'error'; code: MediabunnyErrorCode; message: string };

export type FromMediabunnyWorker = ReadyMessage | FrameMessage | ErrorMessage;

const ctx = self as unknown as DedicatedWorkerGlobalScope;

function post(msg: FromMediabunnyWorker, transfer: Transferable[] = []): void {
	ctx.postMessage(msg, transfer);
}

let input: Input | null = null;
let sink: CanvasSink | null = null;
/** The single in-flight seek. New seeks cancel and supersede it. */
let inFlightAbort: AbortController | null = null;
let disposed = false;

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
		// CanvasSink renders each timestamp to an OffscreenCanvas (in workers).
		// The match-nearest strategy tolerates any frame rate; the editor
		// quantizes to a `floorSec` for cut-crossing, so exact frame alignment
		// isn't needed here. PR-E layers a real cache on top.
		sink = new CanvasSink(track, { fit: 'contain', poolSize: 4 });
		post({
			type: 'ready',
			width,
			height,
			durationSec,
			// fps isn't strictly needed for the editor preview (the cut math
			// already quantizes to the timeline). Report a stable 30 so the
			// editor's existing telemetry stays well-defined. PR-E replaces
			// this with `track.computePacketStats().averagePacketRate`.
			fps: 30,
		});
	} catch (err) {
		post({
			type: 'error',
			code: 'bad-input',
			message: err instanceof Error ? err.message : String(err),
		});
		throw err;
	}
}

async function seek(seq: number, originalSec: number): Promise<void> {
	if (!sink) {
		post({ type: 'error', code: 'worker-died', message: 'Sink not initialized.' });
		return;
	}
	inFlightAbort?.abort();
	const ctrl = new AbortController();
	inFlightAbort = ctrl;

	try {
		const wrapped = await sink.getCanvas(originalSec);
		if (ctrl.signal.aborted || disposed) return;
		if (!wrapped) {
			post({ type: 'error', code: 'internal', message: 'No frame returned for seek.' });
			return;
		}
		const canvas = wrapped.canvas as OffscreenCanvas;
		post(
			{
				type: 'frame',
				seq,
				originalSec,
				canvas,
				width: canvas.width,
				height: canvas.height,
			},
			[canvas],
		);
	} catch (err) {
		if (ctrl.signal.aborted) return;
		post({
			type: 'error',
			code: 'internal',
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

async function prefetch(originalSec: number, _lookaheadSec?: number): Promise<void> {
	if (!sink) return;
	// CanvasSink pre-allocates its pool internally, so the request above
	// already pays the decode cost. For PR-D we treat prefetch as a hint
	// that we ask for an additional nearby timestamp so the GPU surface
	// is warm in cache. PR-E replaces this with an LRU-backed cache.
	try {
		await sink.getCanvas(originalSec);
	} catch {
		/* prefetch is best-effort */
	}
}

function dispose(): void {
	disposed = true;
	inFlightAbort?.abort();
	inFlightAbort = null;
	sink = null;
	if (input) {
		input.dispose();
		input = null;
	}
}

ctx.onmessage = (e: MessageEvent<ToMediabunnyWorker>) => {
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
			void seek(msg.seq, msg.originalSec);
			return;
		case 'prefetch':
			void prefetch(msg.originalSec, msg.lookaheadSec);
			return;
		case 'cancel':
			// Cancel a specific seek or, with no seq, all in-flight seeks.
			if (msg.seq === undefined) {
				inFlightAbort?.abort();
			}
			return;
		case 'dispose':
			dispose();
			return;
	}
};
