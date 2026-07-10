/**
 * Annotation rendering maths: arrow-head geometry, stroke dash patterns, and
 * blur-variant tint colours. The imperative drawing stays in AnnotationOverlay.
 */

import type { HandleName } from "$lib/annotations/hit";
import type { ZoomTransform } from "$lib/annotations/eval";

export type StrokeStyle = "solid" | "dashed" | "dotted";

/** CSS-px half-size of an annotation resize handle (also the grab-slop base). */
export const HANDLE_RADIUS_PX = 5.5;
/** CSS-px corner radius drawn on annotation resize handles. */
export const HANDLE_CORNER_PX = 2;

/** Zoom transform that maps UV straight through, used for frame-anchored annotations, which ignore zoom. */
export const IDENTITY_ZOOM: ZoomTransform = { scale: 1, cx: 0.5, cy: 0.5 };

/** Canvas dash array for a stroke style, scaled by stroke width. Empty = solid. */
export function strokeDashPattern(
	style: StrokeStyle | undefined,
	strokePx: number,
): number[] {
	if (style === "dashed") return [8 * strokePx, 6 * strokePx];
	if (style === "dotted") return [2 * strokePx, 4 * strokePx];
	return [];
}

/**
 * Tint overlay for a blur annotation variant, or null when there's no tint.
 * The tint alpha tracks strength (0.15 → 0.95) and is multiplied by master
 * opacity, mirroring the export exactly (ffmpeg.rs build_annotation_blur_complex:
 * `base_alpha = 0.15 + 0.80*strength`, `× opacity`, glass grey wash past 0.6).
 * `color` variant parses a `#rrggbb` (with/without `#`); invalid colour → null.
 */
export function blurTint(
	variant: string,
	tintColor: string,
	strength: number,
	opacity = 1,
): string | null {
	const s = Math.max(0, Math.min(1, strength));
	const o = Math.max(0, Math.min(1, opacity));
	const a = ((0.15 + 0.8 * s) * o).toFixed(3);
	if (variant === "white") return `rgba(255,255,255,${a})`;
	if (variant === "black") return `rgba(0,0,0,${a})`;
	if (variant === "color") {
		const m = /^#?([0-9a-fA-F]{6})$/.exec(tintColor.trim());
		if (m) {
			const v = parseInt(m[1], 16);
			return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${a})`;
		}
		return null;
	}
	// glass: no tint until pushed hard, then a faint grey wash so it still redacts.
	if (s > 0.6) return `rgba(128,128,128,${(((s - 0.6) * 0.6) * o).toFixed(3)})`;
	return null;
}

/**
 * Multiply a CSS colour's alpha by `factor`, returning an `rgba(...)` string.
 * Handles `#rrggbb`, `#rrggbbaa`, and `rgb()/rgba()`; any other form (tokens,
 * named colours) is returned unchanged. Used to bake glow opacity into the
 * shadow colour so the cast glow dims without fading the shape itself, which
 * is how the export treats `glow.opacity` (cursor_export.rs draw_*_shadow).
 */
export function withAlpha(color: string, factor: number): string {
	const f = Math.max(0, Math.min(1, factor));
	const c = color.trim();
	const hex6 = /^#?([0-9a-fA-F]{6})$/.exec(c);
	if (hex6) {
		const v = parseInt(hex6[1], 16);
		return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${f.toFixed(3)})`;
	}
	const hex8 = /^#?([0-9a-fA-F]{6})([0-9a-fA-F]{2})$/.exec(c);
	if (hex8) {
		const v = parseInt(hex8[1], 16);
		const a = (parseInt(hex8[2], 16) / 255) * f;
		return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${a.toFixed(3)})`;
	}
	const rgb = /^rgba?\(([^)]+)\)$/.exec(c);
	if (rgb) {
		const p = rgb[1].split(",").map((s) => s.trim());
		const a = (p[3] !== undefined ? parseFloat(p[3]) : 1) * f;
		return `rgba(${p[0]},${p[1]},${p[2]},${a.toFixed(3)})`;
	}
	return c;
}

export interface Point {
	x: number;
	y: number;
}

export interface ArrowGeometry {
	/** Where the shaft ends (base of the head). */
	lineEnd: Point;
	/** The arrow tip (p2). */
	tip: Point;
	/** The two base corners of the head triangle. */
	left: Point;
	right: Point;
}

/**
 * Arrow shaft + head triangle geometry from endpoints, stroke width, and head
 * size (fraction of length). Returns null for a degenerate (sub-pixel) arrow.
 */
export function arrowGeometry(
	p1: Point,
	p2: Point,
	strokePx: number,
	headSize: number,
): ArrowGeometry | null {
	const dx = p2.x - p1.x;
	const dy = p2.y - p1.y;
	const len = Math.hypot(dx, dy);
	if (len < 1) return null;
	const headLen = Math.max(strokePx * 2, headSize * len);
	const headWidth = headLen * 0.7;
	const ux = dx / len;
	const uy = dy / len;
	const lineEndX = p2.x - ux * headLen;
	const lineEndY = p2.y - uy * headLen;
	const nx = -uy;
	const ny = ux;
	return {
		lineEnd: { x: lineEndX, y: lineEndY },
		tip: { x: p2.x, y: p2.y },
		left: { x: lineEndX + nx * headWidth * 0.5, y: lineEndY + ny * headWidth * 0.5 },
		right: { x: lineEndX - nx * headWidth * 0.5, y: lineEndY - ny * headWidth * 0.5 },
	};
}

/**
 * Emit a rounded-rectangle sub-path into `ctx` (caller owns beginPath/fill/
 * stroke). Radius is clamped to half the shorter side so a full-radius box
 * rounds cleanly regardless of dimensions. Path-building only, no ctx state
 * is mutated beyond the current path.
 */
export function roundRectPath(
	ctx: CanvasRenderingContext2D,
	x: number,
	y: number,
	w: number,
	h: number,
	r: number,
): void {
	const maxR = Math.min(Math.abs(w) / 2, Math.abs(h) / 2);
	const rr = Math.min(r, maxR);
	ctx.moveTo(x + rr, y);
	ctx.lineTo(x + w - rr, y);
	ctx.quadraticCurveTo(x + w, y, x + w, y + rr);
	ctx.lineTo(x + w, y + h - rr);
	ctx.quadraticCurveTo(x + w, y + h, x + w - rr, y + h);
	ctx.lineTo(x + rr, y + h);
	ctx.quadraticCurveTo(x, y + h, x, y + h - rr);
	ctx.lineTo(x, y + rr);
	ctx.quadraticCurveTo(x, y, x + rr, y);
	ctx.closePath();
}

/**
 * CSS cursor for a hovered annotation handle. Distinct from the focus overlay's
 * map: annotations add the `tool` (placement) and arrow `p1`/`p2` states, and
 * body hovers read as `grab` rather than `move`.
 */
export function cursorForHandle(h: HandleName | "tool" | null): string {
	if (h === "tool") return "crosshair";
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
		case "p1":
		case "p2":
			return "crosshair";
		case "body":
			return "grab";
		default:
			return "default";
	}
}
