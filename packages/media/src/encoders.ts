/**
 * Small output encoders for the formats MediaBunny / WebCodecs don't write
 * directly: GIF (gifenc), WAV (raw PCM, no library), MP3 (lamejs), and ZIP
 * (fflate) for multi-file output like extracted frames.
 *
 * PR-A: stub interface only (defines the contract). PR-B relocates the
 * implementation from apps/web/src/lib/tools/encoders.ts unchanged. Until
 * PR-B, callers should keep importing from apps/web's local copy.
 */

/** A GIF writer backed by gifenc. Quantizes each frame to a 256-colour palette. */
export interface GifWriter {
	/** Add one frame from RGBA pixels; `delayMs` is how long it shows. */
	addFrame(
		rgba: Uint8Array | Uint8ClampedArray,
		width: number,
		height: number,
		delayMs: number,
	): void;
	/** Finish and return the GIF bytes. */
	finish(): Uint8Array;
}

/** Create a fresh GIF writer. */
export function createGifWriter(): GifWriter {
	throw new Error('createGifWriter is not yet implemented — lands in PR-B');
}

/** Encode planar float channels to a 16-bit PCM WAV file. */
export function encodeWav(_channels: Float32Array[], _sampleRate: number): Uint8Array {
	throw new Error('encodeWav is not yet implemented — lands in PR-B');
}

/** Encode planar float channels to an MP3 file at the given bitrate. */
export function encodeMp3(
	_channels: Float32Array[],
	_sampleRate: number,
	_kbps: number,
): Uint8Array {
	throw new Error('encodeMp3 is not yet implemented — lands in PR-B');
}

/** Zip a set of already-compressed files (images) with no extra compression. */
export function zipFiles(_files: Record<string, Uint8Array>): Uint8Array {
	throw new Error('zipFiles is not yet implemented — lands in PR-B');
}
