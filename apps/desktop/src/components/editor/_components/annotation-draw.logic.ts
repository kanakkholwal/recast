/**
 * Annotation rendering maths: arrow-head geometry, stroke dash patterns, and
 * blur-variant tint colours. The imperative drawing stays in AnnotationOverlay.
 */

export type StrokeStyle = "solid" | "dashed" | "dotted";

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
