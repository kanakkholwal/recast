// Canvas geometry shared with (mirrored in) the Rust export pipeline. Both MUST
// agree on canvas dims and comp position or preview and exported MP4 disagree on
// framing.
//
// Model:
//   1. comp = source dims + uniform `padding` on every side (the v1 rectangle).
//   2. outputAspect `source` → canvas equals comp.
//   3. Otherwise extend whichever axis is too short for the target aspect; the
//      comp stays centred, new bars (one axis only) filled by the background.
//
// We never CROP the comp, only extend around it, so every annotation, cursor,
// and focus region keeps its source-pixel coordinates across aspect changes.

import { aspectRatio, type OutputAspect } from "../stores/editor-store.svelte";

export interface CanvasGeometry {
	/** Final canvas width in source pixels. */
	canvasW: number;
	/** Final canvas height in source pixels. */
	canvasH: number;
	/** Top-left of the source video inside the canvas (NOT the comp). */
	videoX: number;
	videoY: number;
	/** The source video's own dimensions (passthrough, convenience). */
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
export function paddingPxFromPercent(srcW: number, srcH: number, paddingPct: number): number {
	const pct = Math.max(0, Math.min(20, paddingPct));
	const shorter = Math.min(srcW, srcH);
	return Math.round((shorter * pct) / 100);
}

/**
 * Compute canvas geometry for a source size, padding %, and aspect target. Pure.
 * Dims are even-integer-aligned because downstream encoders require it.
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

	// Round up to the next even pixel (H.264 refuses odd dims) without ever
	// shrinking below the comp.
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
