/**
 * Pure planning for the browser-side export encoder: frame count and the
 * WebCodecs video-encoding config. The offline renderer composites each frame
 * through the engine and hands it to MediaBunny's encoder; Rust FFmpeg then
 * muxes the resulting video with the processed audio (`-c:v copy`), so the
 * config here IS the final video quality.
 */

import {
	QUALITY_HIGH,
	QUALITY_LOW,
	QUALITY_MEDIUM,
	QUALITY_VERY_HIGH,
	type VideoEncodingConfig,
} from "@recast/media/mediabunny";

export type ExportQuality = "low" | "medium" | "high" | "max";

/** Output frame count for a timeline of `outputDurationSec` at `fps`. */
export function exportFrameCount(fps: number, outputDurationSec: number): number {
	if (!(fps > 0) || !(outputDurationSec > 0)) return 0;
	return Math.max(1, Math.round(outputDurationSec * fps));
}

/** Output-time (seconds) of frame `index` at `fps` — the timestamp handed to the
 *  encoder and mapped back through the time-map to sample the source. */
export function exportFrameTime(index: number, fps: number): number {
	return fps > 0 ? index / fps : 0;
}

/** H.264 (mp4) encoding config for the quality tier. Passing a subjective Quality
 *  makes MediaBunny ≥1.52 encode at a constant-quality quantizer (like x264 CRF)
 *  rather than a fixed bitrate — steadier quality-per-size, closer to the Rust path. */
export function videoEncodingConfigFor(
	quality: ExportQuality,
	keyFrameIntervalSec = 2,
): VideoEncodingConfig {
	const level =
		quality === "low"
			? QUALITY_LOW
			: quality === "medium"
				? QUALITY_MEDIUM
				: quality === "max"
					? QUALITY_VERY_HIGH
					: QUALITY_HIGH;
	return { codec: "avc", quality: level, keyFrameInterval: keyFrameIntervalSec };
}
