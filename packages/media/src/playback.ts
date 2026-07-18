/**
 * Worker-bridged media playback. The desktop editor's preview pipeline
 * talks to this surface; it talks to MediaBunny running inside a Web Worker.
 *
 * PR-A: API surface only (stub). PR-D lands the real implementation with the
 * MediaWorker class, IndexedDB-backed decoded-frame cache, and feature flag.
 *
 * See packages/media/PLAN.md PR-D for the full wiring.
 */

import type { MediaSource } from './input';

/**
 * A playback source exposes the minimum surface the editor's preview loop
 * needs: seek to a timestamp, get a decoded frame for the current timestamp,
 * prefetch frames around a target (for smooth scrub / cut-cross), and clean
 * up. Frames are owned by the consumer; the source MUST NOT close a
 * `VideoFrame` before the consumer returns it.
 */
export interface PlaybackSource {
	/** Current playhead timestamp, in seconds, on the ORIGINAL-recording clock. */
	readonly currentTime: number;
	/** Duration of the source in seconds, or `null` if not yet known. */
	readonly duration: number | null;
	/** True once the source is open and the first frame is decodable. */
	readonly isReady: boolean;

	/** Seek to `seconds` (original clock) and resolve with the frame at that point. */
	seek(seconds: number, signal?: AbortSignal): Promise<PlaybackFrame>;

	/** Prefetch frames around `seconds` so a subsequent scrub or cut-cross is instant. */
	prefetchAround(seconds: number, lookaheadSeconds?: number): Promise<void>;

	/** Subscribe to `event`. Returns an unsubscribe function. */
	on(event: 'ready' | 'error' | 'ended', handler: (payload: PlaybackEvent) => void): () => void;

	/** Stop decoding, release GPU buffers and the worker. Idempotent. */
	dispose(): Promise<void>;
}

/** A frame returned from `PlaybackSource.seek`. The consumer owns it and must call `release()` when done. */
export interface PlaybackFrame {
	/** The decoded frame. MUST NOT be closed by the producer (this source). */
	readonly frame: VideoFrame;
	/** Timestamp in seconds on the original-recording clock. */
	readonly seconds: number;
	/** Hand the frame back to the source; safe to call once. */
	release(): void;
}

/** Lifecycle events emitted by a `PlaybackSource`. */
export type PlaybackEvent =
	| { kind: 'ready'; duration: number }
	| { kind: 'error'; error: Error }
	| { kind: 'ended' };

/**
 * Open `source` as a worker-bridged playback source. The returned source is
 * ready to `seek` once the `'ready'` event fires (or once `isReady` flips).
 *
 * PR-A: stub. PR-D wires the MediaWorker + MediaBunny Input + CanvasSink
 * pipeline.
 */
export async function openMediaSource(_source: MediaSource): Promise<PlaybackSource> {
	throw new Error('openMediaSource is not yet implemented — lands in PR-D');
}

/** Seek to `seconds` on an already-open playback source. Convenience for one-shot seeks. */
export async function seekTo(
	_source: PlaybackSource,
	_seconds: number,
	_signal?: AbortSignal,
): Promise<PlaybackFrame> {
	throw new Error('seekTo is not yet implemented — lands in PR-D');
}

/** Prefetch frames around `seconds` on an already-open playback source. */
export async function prefetchAround(
	_source: PlaybackSource,
	_seconds: number,
	_lookaheadSeconds?: number,
): Promise<void> {
	throw new Error('prefetchAround is not yet implemented — lands in PR-D');
}

/** Clear the shared decoded-frame cache. Returns the number of entries evicted. */
export async function evictCache(): Promise<number> {
	throw new Error('evictCache is not yet implemented — lands in PR-E');
}

/** Snapshot of the cache's current usage; useful for diagnostics and the Settings UI. */
export function cacheStats(): {
	entryCount: number;
	bytes: number;
	capBytes: number;
	oldestEntryAgeMs: number;
} {
	throw new Error('cacheStats is not yet implemented — lands in PR-E');
}
