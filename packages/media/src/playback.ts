/**
 * Worker-bridged media playback for the editor's preview pipeline. The desktop
 * app's main thread talks to this surface; this module talks to a Web Worker
 * that owns the MediaBunny `Input` + `CanvasSink` lifecycle.
 *
 * Contract (packages/media/REQUIREMENTS.md §5):
 * - Async exports that do I/O take an `AbortSignal`.
 * - Frames are owned by the shared cache, which closes them on eviction; the
 *   consumer uploads and does not close.
 * - Demux and decode run in the worker, never on the main thread.
 */

import { MediaError } from './errors';

/**
 * A media input: a URL (Tauri `asset:` on desktop, range-capable HTTP on web)
 * or a `File`/`Blob` the user picked.
 */
export type MediaSource = string | File | Blob;

/** Opaque token a consumer can pass back to `releaseFrame` / `prefetch`. */
export type FrameHandle = number;

/**
 * Lifecycle events emitted by a `PlaybackSource`. The consumer subscribes
 * via `source.on(...)` and unsubscribes via the returned cleanup function.
 */
export type PlaybackEvent =
	| { kind: 'ready'; duration: number }
	| { kind: 'error'; error: Error }
	| { kind: 'ended' };

/**
 * A frame returned from `PlaybackSource.seek`. The consumer owns it and MUST
 * call `release()` when done (typically synchronously after uploading to
 * WebGL). Failing to release leaks a GPU surface.
 */
export interface PlaybackFrame {
	/** The decoded frame. The consumer reads pixels from it. */
	readonly frame: VideoFrame;
	/** Timestamp in seconds on the original-recording clock. */
	readonly seconds: number;
	/**
	 * Hand the frame back to the source. Safe to call exactly once. The
	 * source closes the underlying `VideoFrame` here.
	 */
	release(): void;
}

/**
 * Subscription handle returned by `PlaybackSource.on`. Call to unsubscribe.
 */
export type Unsubscribe = () => void;

/**
 * The high-level playback surface a consumer (editor preview, conversion
 * pipeline) talks to. One instance per open recording; owned by the
 * consumer; `dispose()` is idempotent.
 */
export interface PlaybackSource {
	/** Current playhead timestamp, in seconds, on the ORIGINAL-recording clock. */
	readonly currentTime: number;
	/** Duration of the source in seconds, or `null` if not yet known. */
	readonly duration: number | null;
	/** True once the source is open and the first frame is decodable. */
	readonly isReady: boolean;

	/**
	 * Seek to `seconds` (original clock) and resolve with the frame at that
	 * point. May resolve to `null` if the frame isn't ready yet (the consumer
	 * should hold the previous frame in that case). Cancellable via
	 * `signal`.
	 */
	seek(seconds: number, signal?: AbortSignal): Promise<PlaybackFrame | null>;

	/**
	 * Pre-decode frames around `seconds` without moving the playhead. Used
	 * by the editor's "scout" pattern to warm the post-cut GOP before the
	 * playhead crosses the cut.
	 */
	prefetchAround(seconds: number, lookaheadSeconds?: number): Promise<void>;

	/** Subscribe to a lifecycle event. Returns an unsubscribe function. */
	on(event: 'ready' | 'error' | 'ended', handler: (payload: PlaybackEvent) => void): Unsubscribe;

	/** Stop decoding, release GPU buffers and the worker. Idempotent. */
	dispose(): Promise<void>;
}

/**
 * Open `source` as a worker-bridged playback source. URL only for now; wrap
 * `Blob`/`File` with `URL.createObjectURL`. The worker loads lazily.
 */
export async function openMediaSource(
	source: MediaSource,
	signal?: AbortSignal,
): Promise<PlaybackSource> {
	if (signal?.aborted) throw new MediaError('cancelled', 'openMediaSource aborted');
	const url = typeof source === 'string' ? source : null;
	if (!url) {
		throw new MediaError(
			'unsupported',
			'openMediaSource currently accepts a URL; wrap Blob/File with URL.createObjectURL',
		);
	}
	const { MediabunnyVideoSource } = await import('./playback/index');
	const impl = await MediabunnyVideoSource.create(url);
	if (signal?.aborted) {
		impl.dispose();
		throw new MediaError('cancelled', 'openMediaSource aborted');
	}
	return adaptToPlaybackSource(impl);
}

/** Seek to `seconds` on an already-open playback source. Convenience wrapper. */
export async function seekTo(
	source: PlaybackSource,
	seconds: number,
	signal?: AbortSignal,
): Promise<PlaybackFrame | null> {
	return source.seek(seconds, signal);
}

/** Prefetch frames around `seconds` on an already-open playback source. */
export async function prefetchAround(
	source: PlaybackSource,
	seconds: number,
	lookaheadSeconds?: number,
): Promise<void> {
	return source.prefetchAround(seconds, lookaheadSeconds);
}

/**
 * Adapt `MediabunnyVideoSource`'s sync poll-based `frameAt` to the async
 * `PlaybackSource` surface, awaiting `onFrame` on a cache miss.
 */
function adaptToPlaybackSource(
	impl: import('./playback/source').MediabunnyVideoSource,
): PlaybackSource {
	let currentTime = 0;
	let disposed = false;
	const listeners = new Map<string, Set<(payload: PlaybackEvent) => void>>();
	const emit = (event: string, payload: PlaybackEvent) => {
		for (const fn of listeners.get(event) ?? []) fn(payload);
	};

	return {
		get currentTime() {
			return currentTime;
		},
		get duration() {
			return impl.durationSec;
		},
		get isReady() {
			return !disposed;
		},

		async seek(seconds: number, signal?: AbortSignal): Promise<PlaybackFrame | null> {
			if (signal?.aborted) throw new MediaError('cancelled', 'seek aborted');
			currentTime = seconds;
			const immediate = impl.frameAt(seconds);
			const wrap = (frame: VideoFrame): PlaybackFrame => ({
				frame,
				seconds,
				// The cache owns this surface and closes it on eviction.
				release: () => {},
			});
			if (immediate) return wrap(immediate);

			// Miss: `frameAt` started a decode — wait for it, then re-poll.
			return new Promise<PlaybackFrame | null>((resolve, reject) => {
				const prev = impl.onFrame;
				let settled = false;
				const cleanup = () => {
					impl.onFrame = prev;
					signal?.removeEventListener('abort', onAbort);
				};
				const onAbort = () => {
					if (settled) return;
					settled = true;
					cleanup();
					reject(new MediaError('cancelled', 'seek aborted'));
				};
				impl.onFrame = () => {
					prev?.();
					if (settled) return;
					settled = true;
					cleanup();
					const frame = impl.frameAt(seconds);
					resolve(frame ? wrap(frame) : null);
				};
				signal?.addEventListener('abort', onAbort, { once: true });
			});
		},

		async prefetchAround(seconds: number, _lookaheadSeconds?: number): Promise<void> {
			impl.prefetch(seconds);
		},

		on(event, handler) {
			const set = listeners.get(event) ?? new Set();
			set.add(handler);
			listeners.set(event, set);
			return () => set.delete(handler);
		},

		async dispose(): Promise<void> {
			if (disposed) return;
			disposed = true;
			impl.dispose();
			emit('ended', { kind: 'ended' });
			listeners.clear();
		},
	};
}

/** Clear every entry in the shared decoded-frame cache. */
export async function evictCache(): Promise<number> {
	const { getFrameCache } = await import('./cache');
	return getFrameCache().evictCache();
}

/** Snapshot of the shared decoded-frame cache's current usage. */
export async function cacheStats(): Promise<{
	entryCount: number;
	bytes: number;
	capBytes: number;
	oldestEntryAgeMs: number;
}> {
	const { getFrameCache } = await import('./cache');
	return getFrameCache().cacheStats();
}

// Audio scheduling math, shared with the desktop engine.
export type { Region, ScheduledChunk } from './audio/schedule';
