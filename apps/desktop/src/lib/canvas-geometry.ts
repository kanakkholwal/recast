// Canvas geometry helper shared by the editor preview and (mirrored in)
// the Rust export pipeline. Both must agree on canvas dimensions and the
// position of the source-plus-padding inside that canvas, otherwise
// preview and exported MP4 disagree on framing.
//
// The model:
//   1. compW × compH = source video dims plus uniform `padding` on every
//      side (the v1 "comp" rectangle).
//   2. If outputAspect is `source`, the final canvas equals comp.
//   3. Otherwise we extend whichever axis is too short to satisfy the
//      target aspect ratio. The comp stays centred; the new bars on
//      either side (horizontal or vertical, never both) get filled by
//      the chosen background.
//
// We never CROP the comp — only extend the canvas around it. That keeps
// every annotation, cursor, focus region, and shadow in their original
// source-pixel coordinates, so re-targeting between aspect ratios doesn't
// invalidate any per-source data.

import { aspectRatio, type OutputAspect } from "$lib/stores/editor-store.svelte";

export interface CanvasGeometry {
	/** Final canvas width in source pixels. */
	canvasW: number;
	/** Final canvas height in source pixels. */
	canvasH: number;
	/** Top-left of the source video inside the canvas (NOT the comp). */
	videoX: number;
	videoY: number;
	/** The source video's own dimensions (passthrough — convenience). */
	videoW: number;
	videoH: number;
	/** Padding around the source itself (the v1 uniform value). */
	paddingPx: number;
	/** Comp rectangle (source + uniform padding) inside the canvas. */
	compX: number;
	compY: number;
	compW: number;
	compH: number;
}

/** Convert padding-percent (0..20 of the shorter source edge) to pixels. */
export function paddingPxFromPercent(
	srcW: number,
	srcH: number,
	paddingPct: number,
): number {
	const pct = Math.max(0, Math.min(20, paddingPct));
	const shorter = Math.min(srcW, srcH);
	return Math.round((shorter * pct) / 100);
}

/**
 * Compute the canvas geometry for a given source size, padding %, and
 * aspect target. Pure — same inputs always produce the same output.
 *
 * Source pixels are integer-aligned because downstream encoders (FFmpeg,
 * the WebGL render buffer) are happier with even integer dims.
 */
export function computeCanvasGeometry(
	srcW: number,
	srcH: number,
	paddingPct: number,
	outputAspect: OutputAspect,
): CanvasGeometry {
	const paddingPx = paddingPxFromPercent(srcW, srcH, paddingPct);
	const compW = srcW + paddingPx * 2;
	const compH = srcH + paddingPx * 2;

	const target = aspectRatio(outputAspect);
	let canvasW = compW;
	let canvasH = compH;
	if (target !== null && compW > 0 && compH > 0) {
		const compAspect = compW / compH;
		if (compAspect > target) {
			// Comp is wider than target → extend HEIGHT (top/bottom bars).
			canvasW = compW;
			canvasH = Math.round(compW / target);
		} else if (compAspect < target) {
			// Comp is narrower than target → extend WIDTH (side bars).
			canvasW = Math.round(compH * target);
			canvasH = compH;
		}
	}

	// Even-dim alignment. Some H.264 profiles refuse odd dims, and the
	// FFmpeg pad filter is happier when the offset is integer. The +1/&~1
	// rounding lifts to the next even pixel without ever shrinking below
	// the comp.
	canvasW = (canvasW + 1) & ~1;
	canvasH = (canvasH + 1) & ~1;

	const compX = Math.round((canvasW - compW) / 2);
	const compY = Math.round((canvasH - compH) / 2);
	const videoX = compX + paddingPx;
	const videoY = compY + paddingPx;

	return {
		canvasW,
		canvasH,
		videoX,
		videoY,
		videoW: srcW,
		videoH: srcH,
		paddingPx,
		compX,
		compY,
		compW,
		compH,
	};
}
