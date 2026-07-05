/**
 * Focus/zoom-region overlay geometry: the UV-space box a region occupies, its
 * resize-handle positions, hit-testing, and the cursor for a handle.
 *
 * NOTE: this is the zoom-region editor's own source-space geometry — it does NOT
 * use the zoom-aware/aspect-aware helpers in `$lib/annotations/uv.ts` (those are
 * for annotations, which live in zoomed space). Keep the two separate.
 */

import { framePaddingPixels } from "$lib/editor/frame-padding";
import type { VideoMetadata } from "$lib/stores/editor-store.svelte";

type Meta = Pick<VideoMetadata, "width" | "height"> | null | undefined;

/** Zoom-scale clamp for the focus region editor. */
export const MIN_SCALE = 1.05;
export const MAX_SCALE = 3;

export type HandleName =
	| "nw"
	| "n"
	| "ne"
	| "e"
	| "se"
	| "s"
	| "sw"
	| "w"
	| "body";

export interface Box {
	x: number;
	y: number;
	w: number;
	h: number;
}

/** Half-size (px, pre-DPR) of a resize handle; also the draw size. */
export const HANDLE_RADIUS_PX = 6;

/** UV-space box for a zoom region: a `1/scale`-side square centred on (centerX, centerY), clamped inside [0,1]². */
export function regionBox(r: {
	scale: number;
	centerX: number;
	centerY: number;
}): Box {
	const s = Math.max(1.001, r.scale);
	const w = 1 / s;
	const h = 1 / s;
	const cx = Math.min(Math.max(r.centerX, w / 2), 1 - w / 2);
	const cy = Math.min(Math.max(r.centerY, h / 2), 1 - h / 2);
	return { x: cx - w / 2, y: cy - h / 2, w, h };
}

/** The eight resize-handle anchor points around a px-space rect. */
export function handlePositions(
	x: number,
	y: number,
	w: number,
	h: number,
): Record<Exclude<HandleName, "body">, { x: number; y: number }> {
	return {
		nw: { x, y },
		n: { x: x + w / 2, y },
		ne: { x: x + w, y },
		e: { x: x + w, y: y + h / 2 },
		se: { x: x + w, y: y + h },
		s: { x: x + w / 2, y: y + h },
		sw: { x, y: y + h },
		w: { x, y: y + h / 2 },
	};
}

/** Which handle (or "body") a point hits in a px-space rect, or null. `dpr` scales the grab slop per display. */
export function hitTestHandle(
	pt: { x: number; y: number },
	x: number,
	y: number,
	w: number,
	h: number,
	dpr: number,
): HandleName | null {
	const slop = HANDLE_RADIUS_PX * dpr + 2 * dpr;
	const handles = handlePositions(x, y, w, h);
	for (const [name, p] of Object.entries(handles)) {
		if (Math.abs(pt.x - p.x) <= slop && Math.abs(pt.y - p.y) <= slop) {
			return name as HandleName;
		}
	}
	if (pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h) return "body";
	return null;
}

/** Composition (frame + uniform padding) width in source pixels. */
export function compW(metadata: Meta, paddingPercent: number): number {
	if (!metadata) return 0;
	const paddingPx = framePaddingPixels(paddingPercent, metadata);
	return metadata.width + paddingPx * 2;
}

/** Canvas device-px rect of the video region inside the padded canvas (mirror of the shader). */
export function videoRectPx(
	canvasW: number,
	canvasH: number,
	metadata: Meta,
	paddingPercent: number,
): Box {
	const total = compW(metadata, paddingPercent);
	const sourcePaddingPx = metadata ? framePaddingPixels(paddingPercent, metadata) : 0;
	const padPx = total > 0 ? (sourcePaddingPx / total) * canvasW : 0;
	return { x: padPx, y: padPx, w: canvasW - 2 * padPx, h: canvasH - 2 * padPx };
}

/** Video-region UV → canvas px (no zoom; the region editor works in source space). */
export function uvToCanvas(ux: number, uy: number, rect: Box): { x: number; y: number } {
	return { x: rect.x + ux * rect.w, y: rect.y + uy * rect.h };
}

/** Canvas px → video-region UV (inverse of uvToCanvas). */
export function canvasToUV(cx: number, cy: number, rect: Box): { x: number; y: number } {
	if (rect.w <= 0 || rect.h <= 0) return { x: 0, y: 0 };
	return { x: (cx - rect.x) / rect.w, y: (cy - rect.y) / rect.h };
}

/**
 * New zoom-region scale + centre from dragging a resize handle to UV point `uv`.
 * Rebuilds the focus rect from the dragged edge, keeps it square (uniform zoom)
 * by taking the larger dimension, clamps scale into `[min, max]`, then re-centres
 * on the rect midpoint and clamps the centre so the rect stays inside [0,1]².
 */
export function resizeFocusRegion(
	handle: HandleName,
	startScale: number,
	startCX: number,
	startCY: number,
	uv: { x: number; y: number },
	bounds: { min: number; max: number },
): { scale: number; cx: number; cy: number } {
	const { min, max } = bounds;
	const halfW0 = 1 / (2 * startScale);
	const halfH0 = 1 / (2 * startScale);
	let x0 = startCX - halfW0;
	let y0 = startCY - halfH0;
	let x1 = startCX + halfW0;
	let y1 = startCY + halfH0;

	if (handle === "w" || handle === "nw" || handle === "sw") x0 = uv.x;
	if (handle === "e" || handle === "ne" || handle === "se") x1 = uv.x;
	if (handle === "n" || handle === "nw" || handle === "ne") y0 = uv.y;
	if (handle === "s" || handle === "sw" || handle === "se") y1 = uv.y;

	const rawW = Math.max(1 / max, Math.abs(x1 - x0));
	const rawH = Math.max(1 / max, Math.abs(y1 - y0));
	const side = Math.min(1, Math.max(rawW, rawH, 1 / max));
	const scale = Math.min(max, Math.max(min, 1 / side));

	const midX = (Math.min(x0, x1) + Math.max(x0, x1)) * 0.5;
	const midY = (Math.min(y0, y1) + Math.max(y0, y1)) * 0.5;
	const half = side * 0.5;
	const cx = Math.min(Math.max(midX, half), 1 - half);
	const cy = Math.min(Math.max(midY, half), 1 - half);

	return { scale, cx, cy };
}

/** CSS cursor for a hovered handle. */
export function cursorForHandle(h: HandleName | null): string {
	switch (h) {
		case "nw":
		case "se":
			return "nwse-resize";
		case "ne":
		case "sw":
			return "nesw-resize";
		case "n":
		case "s":
			return "ns-resize";
		case "e":
		case "w":
			return "ew-resize";
		case "body":
			return "move";
		default:
			return "";
	}
}
