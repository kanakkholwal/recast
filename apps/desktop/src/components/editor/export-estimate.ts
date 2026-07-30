/**
 * What the export will actually produce: real pixel dimensions and a size
 * range. The quality preset is a BOUND, not an output size — a portrait clip
 * at "HD" is 608x1080, so the preset label alone misleads.
 *
 * Mirrors `resolve_export_profile` + `build_output_scale_filter` in
 * src-tauri/src/commands/ffmpeg.rs. Keep the two in lockstep.
 */

import type { ExportFormat, ExportQuality, ExportSpeed } from "$lib/stores/editor-store.svelte";

export interface Resolution {
	width: number;
	height: number;
}

const PRESET_BOUNDS: Record<ExportQuality, Resolution | null> = {
	small: { width: 1280, height: 720 },
	hd: { width: 1920, height: 1080 },
	"4k": { width: 3840, height: 2160 },
	source: null,
};

/** Bits per second per megapixel at 30fps, per preset. Calibrated, not derived. */
const BITRATE_PER_MPX: Record<ExportQuality, number> = {
	small: 2_600_000,
	hd: 4_200_000,
	"4k": 9_000_000,
	source: 6_000_000,
};

const SPEED_FACTOR: Record<ExportSpeed, number> = {
	fast: 1.35,
	balanced: 1,
	quality: 0.78,
};

const FORMAT_FACTOR: Record<ExportFormat, number> = {
	mp4: 1,
	// VP9 lands smaller than x264 at comparable quality.
	webm: 0.82,
	// Palette GIF is dominated by frame count and area, not by the codec knobs.
	gif: 6.5,
};

// FFmpeg's scale rounds to nearest, and only the follow-up
// `trunc(iw/2)*2` pass forces even — flooring first is off by two.
function even(n: number) {
	return Math.max(2, Math.floor(Math.round(n) / 2) * 2);
}

/**
 * Output pixel dimensions, or null when the source size is unknown. Fits inside
 * the preset bound preserving aspect (never upscales), then rounds down to even
 * — yuv420p rejects odd dimensions.
 */
export function outputResolution(
	sourceWidth: number,
	sourceHeight: number,
	quality: ExportQuality,
): Resolution | null {
	if (!(sourceWidth > 0) || !(sourceHeight > 0)) return null;
	const bound = PRESET_BOUNDS[quality];
	if (!bound) return { width: even(sourceWidth), height: even(sourceHeight) };
	const scale = Math.min(1, bound.width / sourceWidth, bound.height / sourceHeight);
	return { width: even(sourceWidth * scale), height: even(sourceHeight * scale) };
}

export interface ByteRange {
	low: number;
	high: number;
}

export interface EstimateInput {
	format: ExportFormat;
	quality: ExportQuality;
	speed: ExportSpeed;
	/** Output duration in seconds, after trim and cuts. */
	seconds: number;
	/** Output dimensions (from {@link outputResolution}). */
	width: number;
	height: number;
	fps: number;
}

/**
 * Rough encoded size range. Deliberately a range: content complexity moves the
 * real figure far more than any setting here, so a single number would be a
 * false promise.
 */
export function estimateExportBytes(input: EstimateInput): ByteRange | null {
	const { format, quality, speed, seconds, width, height, fps } = input;
	if (!(seconds > 0) || !(width > 0) || !(height > 0)) return null;
	const megapixels = (width * height) / 1_000_000;
	const fpsFactor = Math.max(0.35, (fps > 0 ? fps : 30) / 30);
	const bitsPerSecond =
		BITRATE_PER_MPX[quality] * megapixels * fpsFactor * SPEED_FACTOR[speed] * FORMAT_FACTOR[format];
	const bytes = (bitsPerSecond * seconds) / 8;
	return { low: Math.round(bytes * 0.65), high: Math.round(bytes * 1.5) };
}

function unitize(bytes: number): { value: number; unit: "KB" | "MB" | "GB" } {
	if (bytes >= 1_000_000_000) return { value: bytes / 1_000_000_000, unit: "GB" };
	if (bytes >= 1_000_000) return { value: bytes / 1_000_000, unit: "MB" };
	return { value: bytes / 1_000, unit: "KB" };
}

// Two significant figures at most: the range is already +-40%, so a decimal
// on megabytes is false precision.
function round(value: number, unit: "KB" | "MB" | "GB") {
	if (unit === "GB" && value < 10) return Number(value.toFixed(1));
	return Math.round(value);
}

/** "4–9 MB", or "~5 MB" when both ends round to the same figure. */
export function formatByteRange(range: ByteRange | null): string | null {
	if (!range) return null;
	const high = unitize(range.high);
	const low = unitize(range.low);
	const lowValue = round(low.unit === high.unit ? low.value : range.low / 1_000_000, high.unit);
	const highValue = round(high.value, high.unit);
	if (low.unit === high.unit && lowValue === highValue) return `~${highValue} ${high.unit}`;
	return `${lowValue}–${highValue} ${high.unit}`;
}
