/** Drag-rect + toolbar geometry for the region-picker overlay. */

import type { RegionRect } from "$lib/recorder-types";

export type Rect = { x: number; y: number; w: number; h: number };

/** Toolbar footprint used to keep it inside the viewport near an edge. */
export const TOOLBAR_W = 240;
export const TOOLBAR_H = 36;

/** Normalise two pointer corners into a top-left origin + positive size. */
export function rectFromPoints(x0: number, y0: number, x1: number, y1: number): Rect {
	return {
		x: Math.min(x0, x1),
		y: Math.min(y0, y1),
		w: Math.abs(x1 - x0),
		h: Math.abs(y1 - y0),
	};
}

/**
 * Local rect → virtual-desktop `RegionRect` (+ label): shift by the overlay's
 * origin and scale to physical pixels, which is what the Rust resolver expects.
 */
export function toRegionPayload(
	rect: Rect,
	origin: { x: number; y: number },
	dpr: number,
): RegionRect & { label: string } {
	const width = Math.round(rect.w * dpr);
	const height = Math.round(rect.h * dpr);
	return {
		x: Math.round((rect.x + origin.x) * dpr),
		y: Math.round((rect.y + origin.y) * dpr),
		width,
		height,
		label: `Area ${width}×${height}`,
	};
}

/**
 * Toolbar position, clamped to the viewport so it stays reachable when the
 * selection lands near the bottom or right edge. Drops above the rect when it
 * would overflow the bottom.
 */
export function clampToolbar(rect: Rect, vw: number, vh: number): { left: number; top: number } {
	const desiredTop = rect.y + rect.h + 6;
	const top = desiredTop + TOOLBAR_H + 8 > vh ? Math.max(8, rect.y - TOOLBAR_H - 6) : desiredTop;
	const left = Math.max(8, Math.min(rect.x, vw - TOOLBAR_W - 8));
	return { left, top };
}

/**
 * What the overlay does once an area is confirmed. The drag interaction is the
 * same either way, so the two flows share one overlay rather than growing a
 * second one that drifts from it.
 */
export type OverlayMode = "record" | "screenshot";

/** The mode a spawned overlay window was opened in; anything else records. */
export function overlayMode(search: string): OverlayMode {
	return new URLSearchParams(search).get("mode") === "screenshot" ? "screenshot" : "record";
}

/** Confirm-button label, naming the action rather than the selection. */
export function confirmLabel(mode: OverlayMode): string {
	return mode === "screenshot" ? "Capture" : "Use area";
}

/** Hint shown before the first drag. */
export function hintLabel(mode: OverlayMode): string {
	return mode === "screenshot" ? "Drag to capture an area" : "Drag to select an area";
}

/**
 * Notification title and body for a saved shot. A refused clipboard copy is
 * named rather than swallowed: the overlay is gone by the time it is noticed.
 */
export function savedMessage(shot: {
	path: string;
	copiedToClipboard?: boolean;
}): [string, string] {
	if (shot.copiedToClipboard) return ["Screenshot saved and copied", shot.path];
	if (shot.copiedToClipboard === false) {
		return ["Screenshot saved", `${shot.path} (could not copy to clipboard)`];
	}
	return ["Screenshot saved", shot.path];
}
