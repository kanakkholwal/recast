/**
 * Headless annotation shape renderer — the paint half of the preview's
 * `AnnotationOverlay.drawAnnotation`, lifted so preview AND export draw through
 * ONE path (parity by construction). Handles rect / ellipse / image / arrow with
 * glow, fill, and stroke. Blur (needs the composited framebuffer) and text (its
 * own layout) are the CALLER's job. Geometry is pre-projected by the caller, so
 * this stays free of zoom/anchor/time.
 */

import {
	arrowGeometry,
	roundRectPath,
	strokeDashPattern,
	withAlpha,
	type Ctx2D,
	type Point,
	type StrokeStyle,
} from "./draw-primitives";
import type { Rect } from "./projection";

export interface RenderStroke {
	width: number; // UV (× rectW → px)
	color: string;
	style?: StrokeStyle;
}

export interface RenderGlow {
	color: string;
	blur: number; // UV (× rectW → px)
	opacity: number;
}

/** Structural view of an annotation the renderer reads. The app's `Annotation`
 *  is assignable to it, so no store type crosses the package boundary. */
export interface RenderableAnnotation {
	stroke: RenderStroke;
	fill: string;
	glow?: RenderGlow;
	kind: {
		kind: string;
		radius?: number;
		path?: string;
		opacity?: number;
		headSize?: number;
	};
}

export interface ShapeImage {
	img: CanvasImageSource;
	ready: boolean;
}

export interface ShapeDeps {
	/** Decoded image for an image annotation, or null when unavailable. */
	getImage(path: string): ShapeImage | null;
	/** Device-pixel ratio for the placeholder stroke; export passes 1. */
	dpr?: number;
}

/** Cast the glow into the ctx shadow (dims the glow, not the shape). */
function applyGlow(ctx: Ctx2D, glow: RenderGlow | undefined, rectW: number): void {
	if (!glow) return;
	ctx.shadowColor = withAlpha(glow.color, glow.opacity);
	ctx.shadowBlur = Math.max(0, glow.blur * rectW);
}

function applyStrokeStyle(ctx: Ctx2D, stroke: RenderStroke, strokePx: number): void {
	ctx.lineWidth = strokePx;
	const style = stroke.style ?? "solid";
	ctx.setLineDash(strokeDashPattern(style, strokePx));
	if (style === "dotted") ctx.lineCap = "round";
}

/** Paint an arrow from projected endpoints. Mirrors AnnotationOverlay.drawArrow. */
export function paintArrow(
	ctx: Ctx2D,
	a: RenderableAnnotation,
	p1: Point,
	p2: Point,
	rectW: number,
	opacity: number,
): void {
	const strokePx = Math.max(2, a.stroke.width * rectW);
	const geo = arrowGeometry(p1, p2, strokePx, a.kind.headSize ?? 0.15);
	if (!geo) return;

	ctx.save();
	ctx.globalAlpha = opacity;
	applyGlow(ctx, a.glow, rectW);
	ctx.strokeStyle = a.stroke.color;
	ctx.fillStyle = a.stroke.color;
	applyStrokeStyle(ctx, a.stroke, strokePx);
	ctx.lineCap = "round";

	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(geo.lineEnd.x, geo.lineEnd.y);
	ctx.stroke();

	ctx.setLineDash([]);
	ctx.beginPath();
	ctx.moveTo(geo.tip.x, geo.tip.y);
	ctx.lineTo(geo.left.x, geo.left.y);
	ctx.lineTo(geo.right.x, geo.right.y);
	ctx.closePath();
	ctx.fill();

	ctx.restore();
}

function paintImage(
	ctx: Ctx2D,
	a: RenderableAnnotation,
	box: Rect,
	rectW: number,
	deps: ShapeDeps,
): void {
	const { x, y, w, h } = box;
	const path = a.kind.path;
	const entry = path ? deps.getImage(path) : null;
	const dpr = deps.dpr ?? 1;
	if (entry?.ready) {
		ctx.save();
		ctx.globalAlpha *= Math.max(0, Math.min(1, a.kind.opacity ?? 1));
		const cornerPx = Math.max(0, (a.kind.radius ?? 0) * Math.min(Math.abs(w), Math.abs(h)));
		if (cornerPx > 0.5) {
			if (ctx.shadowBlur > 0) {
				ctx.beginPath();
				roundRectPath(ctx, x, y, w, h, cornerPx);
				ctx.fill();
				ctx.shadowColor = "transparent";
				ctx.shadowBlur = 0;
			}
			ctx.beginPath();
			roundRectPath(ctx, x, y, w, h, cornerPx);
			ctx.clip();
		}
		try {
			ctx.drawImage(entry.img, x, y, w, h);
		} catch {
			/* not decodable this frame */
		}
		ctx.restore();
	} else {
		ctx.save();
		ctx.fillStyle = "rgba(120, 120, 120, 0.12)";
		ctx.fillRect(x, y, w, h);
		ctx.strokeStyle = "rgba(120, 120, 120, 0.5)";
		ctx.setLineDash([6 * dpr, 4 * dpr]);
		ctx.lineWidth = dpr;
		ctx.strokeRect(x, y, w, h);
		ctx.restore();
	}

	// Border sits on the image (separate from the shape fill/stroke path).
	if (a.stroke.color && a.stroke.color !== "transparent" && a.stroke.width > 0) {
		const cornerPx = Math.max(0, (a.kind.radius ?? 0) * Math.min(Math.abs(w), Math.abs(h)));
		const strokePx = Math.max(1, a.stroke.width * rectW);
		ctx.shadowColor = "transparent";
		ctx.shadowBlur = 0;
		ctx.beginPath();
		if (cornerPx > 0.5) roundRectPath(ctx, x, y, w, h, cornerPx);
		else ctx.rect(x, y, w, h);
		applyStrokeStyle(ctx, a.stroke, strokePx);
		ctx.strokeStyle = a.stroke.color;
		ctx.stroke();
	}
}

/** Paint a box-shaped annotation (rect / ellipse / image) from its projected
 *  pixel box. Mirrors the box branches of AnnotationOverlay.drawAnnotation. */
export function paintBoxAnnotation(
	ctx: Ctx2D,
	a: RenderableAnnotation,
	box: Rect,
	rectW: number,
	opacity: number,
	deps: ShapeDeps,
): void {
	const { x, y, w, h } = box;
	if (w <= 0 || h <= 0) return;

	ctx.save();
	ctx.globalAlpha = opacity;
	applyGlow(ctx, a.glow, rectW);

	ctx.beginPath();
	if (a.kind.kind === "rect") {
		const radius = Math.max(0, (a.kind.radius ?? 0) * Math.min(w, h));
		if (radius > 0) roundRectPath(ctx, x, y, w, h, radius);
		else ctx.rect(x, y, w, h);
	} else if (a.kind.kind === "ellipse") {
		ctx.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
	} else if (a.kind.kind === "image") {
		paintImage(ctx, a, box, rectW, deps);
	}

	if (a.kind.kind !== "image" && a.fill && a.fill !== "transparent") {
		ctx.fillStyle = a.fill;
		ctx.fill();
	}
	if (
		a.kind.kind !== "image" &&
		a.stroke.color &&
		a.stroke.color !== "transparent" &&
		a.stroke.width > 0
	) {
		const strokePx = Math.max(1, a.stroke.width * rectW);
		applyStrokeStyle(ctx, a.stroke, strokePx);
		ctx.strokeStyle = a.stroke.color;
		ctx.stroke();
	}

	ctx.restore();
}
