/**
 * Raw MediaBunny primitives for worker modules that cannot resolve a
 * re-exported class through a barrel. Application code uses the main barrel.
 */

// biome-ignore-all lint/style/noRestrictedImports: sanctioned re-export point.
import { BlobSource, UrlSource } from "mediabunny";
import type { MediaRef } from "./media-ref";

/** Build the `Input` source for a ref. Lives here, not in `media-ref.ts`, so
 *  that module stays importable from the barrel without pulling MediaBunny in. */
export function mediaRefSource(ref: MediaRef): BlobSource | UrlSource {
	return ref.kind === "blob" ? new BlobSource(ref.blob) : new UrlSource(ref.url);
}

export {
	ALL_FORMATS,
	AudioBufferSink,
	AudioBufferSource,
	BlobSource,
	BufferTarget,
	CanvasSink,
	CanvasSource,
	Input,
	Mp4OutputFormat,
	Output,
	QUALITY_HIGH,
	QUALITY_LOW,
	QUALITY_MEDIUM,
	QUALITY_VERY_HIGH,
	UrlSource,
	VideoSampleSink,
} from "mediabunny";
export type {
	InputVideoTrack,
	VideoEncodingConfig,
	WrappedAudioBuffer,
	WrappedCanvas,
} from "mediabunny";
