/**
 * Frame-padding maths: percent → source pixels, clamping, and the legacy
 * pixel-stored → percent migration. Pure (no runes), so it stays importable
 * from `.logic.ts` modules and unit tests without loading the editor store.
 */

import type { VideoMetadata } from "$lib/editor/render-state";

export const MAX_FRAME_PADDING_PERCENT = 20;

export function clampFramePaddingPercent(value: number): number {
	if (!Number.isFinite(value)) return 0;
	return Math.max(0, Math.min(MAX_FRAME_PADDING_PERCENT, value));
}

export function framePaddingPixels(
	paddingPercent: number,
	metadata: Pick<VideoMetadata, "width" | "height"> | null | undefined,
): number {
	if (!metadata?.width || !metadata?.height) return 0;
	const shorterEdge = Math.min(metadata.width, metadata.height);
	const pct = clampFramePaddingPercent(paddingPercent);
	return (pct / 100) * shorterEdge;
}

export function normalizeFramePaddingPercent(
	value: number,
	metadata: Pick<VideoMetadata, "width" | "height"> | null | undefined,
): number {
	if (!Number.isFinite(value)) return 0;
	const nonNegative = Math.max(0, value);
	if (nonNegative <= MAX_FRAME_PADDING_PERCENT) {
		return clampFramePaddingPercent(nonNegative);
	}
	// Legacy projects stored padding as raw pixels.
	if (metadata?.width && metadata?.height) {
		const shorterEdge = Math.min(metadata.width, metadata.height);
		if (shorterEdge > 0) {
			return clampFramePaddingPercent((nonNegative / shorterEdge) * 100);
		}
	}
	return 0;
}
