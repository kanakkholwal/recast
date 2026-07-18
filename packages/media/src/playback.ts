/**
 * Worker-bridged media playback for the editor's preview pipeline. The desktop
 * app's main thread talks to this surface; this module talks to a Web Worker
 * that owns the MediaBunny `Input` + `CanvasSink` lifecycle.
 *
 * PR-D: the landing strip. The worker, the high-level API, and the cache
 * skeleton land here. The IndexedDB-backed decoded-frame cache lands in PR-E.
 *
 * Contract (packages/media/REQUIREMENTS.md §5):
 * - All async exports take an `AbortSignal` so consumers can cancel mid-flight.
 * - `VideoFrame`s returned from `seek` are owned by the consumer; the source
 *   MUST NOT close them. The consumer returns them via `PlaybackFrame.release`.
 * - The worker, the decode pipeline, and the byte-level decoder all live
 *   outside the main thread (REQUIREMENTS.md §4.x).
 */

import { MediaError } from './errors';

/** A file-like thing a user hands us: a `File` (browser/web) or a `Blob`. */
export type MediaSource = File | Blob;

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
 * Open `source` as a worker-bridged playback source. The returned source is
 * ready to `seek` once the `'ready'` event fires (or once `isReady` flips).
 *
 * Implementation note: PR-D ships a basic, non-streaming worker that hands
 * the URL/bytes to MediaBunny's `Input` directly. The IDB-backed decoded-
 * frame cache lands in PR-E; the audio scheduler lands in PR-E.
 */
export async function openMediaSource(_source: MediaSource): Promise<PlaybackSource> {
	throw new MediaError(
		'unsupported',
		'openMediaSource is not yet implemented — lands in PR-D via MediaWorkerClient',
	);
}

/** Seek to `seconds` on an already-open playback source. Convenience wrapper. */
export async function seekTo(
	_source: PlaybackSource,
	_seconds: number,
	_signal?: AbortSignal,
): Promise<PlaybackFrame | null> {
	throw new MediaError('unsupported', 'seekTo is not yet implemented — lands in PR-D');
}

/** Prefetch frames around `seconds` on an already-open playback source. */
export async function prefetchAround(
	_source: PlaybackSource,
	_seconds: number,
	_lookaheadSeconds?: number,
): Promise<void> {
	throw new MediaError('unsupported', 'prefetchAround is not yet implemented — lands in PR-D');
}

/** Clear the shared decoded-frame cache. Stub for PR-D; real in PR-E. */
export async function evictCache(): Promise<number> {
	throw new MediaError('unsupported', 'evictCache is not yet implemented — lands in PR-E');
}

/** Snapshot of the cache's current usage. Stub for PR-D; real in PR-E. */
export function cacheStats(): {
	entryCount: number;
	bytes: number;
	capBytes: number;
	oldestEntryAgeMs: number;
} {
	throw new MediaError('unsupported', 'cacheStats is not yet implemented — lands in PR-E');
}
