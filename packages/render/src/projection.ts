/**
 * Pure UV ↔ canvas projection for the scene/annotation renderer. The zoom
 * transform (scale about a pinned centre) is supplied by the caller — the app
 * evaluates it per frame from easing/time and hands it in, so this stays free of
 * easing + store coupling (the adapter boundary). Preview and export share it.
 */

export interface Rect {
	x: number;
	y: number;
	w: number;
	h: number;
}

/** Zoom applied to video-anchored content: `scale` about pinned `(cx,cy)` in UV.
 *  Frame-anchored content passes the identity (scale 1). */
export interface ZoomTransform {
	scale: number;
	cx: number;
	cy: number;
}

export interface UVPoint {
	x: number;
	y: number;
}

/** Annotation UV → container px, applying the shader's zoom transform. */
export function uvToCanvas(ux: number, uy: number, rect: Rect, zoom: ZoomTransform): UVPoint {
	const preX = (ux - zoom.cx) * zoom.scale + zoom.cx;
	const preY = (uy - zoom.cy) * zoom.scale + zoom.cy;
	return { x: rect.x + preX * rect.w, y: rect.y + preY * rect.h };
}

/** Container px → annotation UV (inverse of {@link uvToCanvas}). */
export function canvasToUV(cx: number, cy: number, rect: Rect, zoom: ZoomTransform): UVPoint {
	if (rect.w <= 0 || rect.h <= 0) return { x: 0, y: 0 };
	const preX = (cx - rect.x) / rect.w;
	const preY = (cy - rect.y) / rect.h;
	return { x: (preX - zoom.cx) / zoom.scale + zoom.cx, y: (preY - zoom.cy) / zoom.scale + zoom.cy };
}

/** Structural view of a box/arrow kind for {@link normaliseBox} — the app's
 *  AnnotationKind union satisfies it without importing the store type. */
export interface NormalisableKind {
	kind: string;
	x?: number;
	y?: number;
	w?: number;
	h?: number;
	x1?: number;
	y1?: number;
	x2?: number;
	y2?: number;
}

/**
 * Normalise a kind's bounding box so width/height are positive. Lets the user
 * drag any of the four diagonals while storage stays canonical.
 */
export function normaliseBox(k: NormalisableKind): Rect {
	if (
		k.kind === "rect" ||
		k.kind === "ellipse" ||
		k.kind === "image" ||
		k.kind === "text" ||
		k.kind === "blur"
	) {
		const kx = k.x ?? 0;
		const ky = k.y ?? 0;
		const kw = k.w ?? 0;
		const kh = k.h ?? 0;
		return { x: Math.min(kx, kx + kw), y: Math.min(ky, ky + kh), w: Math.abs(kw), h: Math.abs(kh) };
	}
	if (k.kind === "arrow") {
		const x1 = k.x1 ?? 0;
		const y1 = k.y1 ?? 0;
		const x2 = k.x2 ?? 0;
		const y2 = k.y2 ?? 0;
		return { x: Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) };
	}
	return { x: 0, y: 0, w: 0, h: 0 };
}
