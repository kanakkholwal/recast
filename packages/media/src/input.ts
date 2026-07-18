/**
 * Open a user-supplied file as a MediaBunny `Input`. The returned `Input` owns
 * the underlying source; callers are responsible for calling `input.dispose()`
 * when done. (PR-B will move the apps/web implementation here unchanged.)
 *
 * Contract (REQUIREMENTS.md §5):
 * - This is the ONLY way consumers should construct an `Input`. Direct
 *   `mediabunny` imports in consumer code are forbidden.
 * - The returned `Input` must be `dispose()`-d by the caller; no leak.
 */

import type { Input } from 'mediabunny';

/** A file-like thing a user hands us: a `File` (browser/web), a `Blob`, or a string URL. */
export type MediaSource = File | Blob | string;

/**
 * Open `source` as a MediaBunny `Input`. Throws `MediaError` with code
 * `bad-input` if the file can't be parsed by any supported demuxer.
 *
 * Note: real implementation lives in PR-B. The stub preserves the signature
 * so consumers can compile against this package today.
 */
export async function openInput(_source: MediaSource): Promise<Input> {
	throw new Error('openInput is not yet implemented — lands in PR-B');
}
