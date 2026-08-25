<script lang="ts">
import { evalOpacity } from "../../lib/annotations/eval";
import { annotationZoom } from "./annotation-projection.logic";
import { nudgeVectorPx } from "./annotation-keys.logic";
import {
	handlePositions,
	hitTestAnnotation,
	hitTestHandle,
	type HandleName,
} from "../../lib/annotations/hit";
import { isEditableTarget } from "../../lib/dom/editable";
import {
	canvasToUV,
	compositionRectPx,
	normaliseBox,
	uvToCanvas,
	videoRectPx,
	type Rect,
} from "../../lib/annotations/uv";
import { snap, snapBox, type SnapAnchor } from "../../lib/annotations/snap";
import {
	constrain45,
	constrainSquare,
	isCornerHandle,
	lockAspect,
} from "../../lib/annotations/resize-constraints";
import { clickPlacedArrow, clickPlacedBox } from "../../lib/annotations/place-defaults";
import { disposeCanvasTokens, selectionPalette } from "../../lib/annotations/canvas-tokens";
import {
	blurTint,
	cursorForHandle,
	HANDLE_CORNER_PX,
	HANDLE_RADIUS_PX,
	IDENTITY_ZOOM,
	roundRectPath,
} from "./annotation-draw.logic";
import { paintArrow, paintBlur, paintBoxAnnotation } from "@recast/render";
import { buildAnnotationSnapAnchors } from "./annotation-snap.logic";
import type {
	Annotation,
	AnnotationAnchor,
	AnnotationKind,
	EditorStore,
} from "../../stores/editor-store.svelte";
import { getEditorServices } from "../../lib/editor/services";
import { onDestroy, onMount } from "svelte";

interface Props {
	store: EditorStore;
	videoEl: HTMLVideoElement | null;
	/** The container that wraps the WebGL preview canvas, which we stretch to fit. */
	targetEl: HTMLElement | null;
	/** The WebGL composite canvas. Used as the source for blur annotations,
	 *  so we can blur the actual rendered frame (background + padding +
	 *  shadow + video) rather than just the bare video. */
	compositeCanvasEl?: HTMLCanvasElement | null;
	/** Original-time position of the frame the compositor actually drew. The
	 *  `<video>` element is NOT that clock on the MediaBunny path (it stays
	 *  paused and is only re-synced past a 0.25s tolerance), so reading it here
	 *  ramped fades and tracked zoom up to a quarter-second off the picture. */
	previewTime?: number;
	/** True when the compositor already painted the annotation artwork. This
	 *  canvas then owns only the editing affordances, or every annotation is
	 *  drawn twice (and a blur is blurred twice). */
	compositorPaintsArtwork?: boolean;
}

let {
	store,
	videoEl,
	targetEl,
	compositeCanvasEl = null,
	previewTime,
	compositorPaintsArtwork = false,
}: Props = $props();

let canvasEl: HTMLCanvasElement | null = $state(null);
let rafHandle: number | null = null;
// Container CSS size, cached from a ResizeObserver so the rAF loop doesn't
// force a layout with getBoundingClientRect() every frame.
let targetSize = { w: 0, h: 0 };

//  Drag / placement state
type DragState =
	| null
	| {
			kind: "move";
			id: string;
			startX: number; // UV (top-left for boxes; x1 for arrows)
			startY: number;
			// For arrows, also keep the second endpoint so we can move both
			// together while preserving the arrow's orientation/length.
			startX2?: number;
			startY2?: number;
			pointerStartUV: { x: number; y: number };
	  }
	| {
			kind: "resize";
			id: string;
			handle: HandleName;
			startBox: { x: number; y: number; w: number; h: number };
	  }
	| {
			kind: "place";
			id: string;
			anchor: { x: number; y: number };
	  };
// $state so `canvasCursor` recomputes when a gesture starts: as a plain local
// its "grabbing" branch could never fire, because only `hoverHandle` re-ran the
// derived and hover is deliberately frozen for the duration of a drag.
let drag: DragState = $state(null);
// Undo is pushed on the first real move of a move/resize drag, not at
// pointer-down, so a pure select-click leaves no no-op entry. Placement
// pushes via addAnnotation, so it starts "already pushed".
let dragUndoPushed = true;
// Active snap guides for the current drag, in UV space. Cleared on
// pointerup. Capped to 4 simultaneous guides to avoid visual noise.
let snapGuides: SnapAnchor[] = $state([]);
// What's under the pointer, used purely for cursor affordance ("grab" on
// body, "nwse-resize" / "ns-resize" / etc on handles). Cleared on leave.
let hoverHandle: HandleName | null | "tool" = $state(null);

// Thin wrappers around shared geometry modules; this file owns rendering +
// interaction state, not the math.
function getDpr(): number {
	return window.devicePixelRatio || 1;
}

function videoRect(): Rect {
	if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
	return videoRectPx(
		canvasEl.width,
		canvasEl.height,
		store.metadata,
		store.padding,
		store.outputAspect,
	);
}

function compRect(): Rect {
	if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
	return compositionRectPx(
		canvasEl.width,
		canvasEl.height,
		store.metadata,
		store.padding,
		store.outputAspect,
	);
}

/** Rect an annotation projects onto: the padded frame when anchored to
 *  "frame", otherwise the video region (which the zoom transform then acts
 *  on). Accepts anything with an optional `anchor` so placement can pass a
 *  bare object. */
function rectFor(a: { anchor?: AnnotationAnchor }): Rect {
	return a.anchor === "frame" ? compRect() : videoRect();
}

function zoomFor(a: { anchor?: AnnotationAnchor }, t: number) {
	return annotationZoom(a.anchor, store.zoomRegions, t, store.focusEnabled);
}

function projectA(a: { anchor?: AnnotationAnchor }, ux: number, uy: number, t: number) {
	return uvToCanvas(ux, uy, rectFor(a), zoomFor(a, t));
}

function unprojectA(a: { anchor?: AnnotationAnchor }, cx: number, cy: number, t: number) {
	return canvasToUV(cx, cy, rectFor(a), zoomFor(a, t));
}

/** True if this annotation should NOT draw on the 2D-canvas overlay. Text
 * lives in a separate HTML layer (TextAnnotationLayer) so the WebView
 * handles glyph rendering and inline edit. */
function isCanvasDrawn(k: AnnotationKind): boolean {
	return k.kind !== "text";
}

function pointerToCanvasPx(e: PointerEvent): { x: number; y: number } {
	if (!canvasEl) return { x: 0, y: 0 };
	const rect = canvasEl.getBoundingClientRect();
	const dpr = getDpr();
	return {
		x: (e.clientX - rect.left) * dpr,
		y: (e.clientY - rect.top) * dpr,
	};
}

function playbackTime(): number {
	return previewTime ?? videoEl?.currentTime ?? store.currentTime;
}

//  Drawing

function drawAnnotation(ctx: CanvasRenderingContext2D, a: Annotation, opacity: number, t: number) {
	// Blur bypasses the fade ramps in preview: a fresh blur (start ≈ currentTime)
	// would ramp from opacity 0 and early-return, and a half-transparent blur
	// copy over the unblurred canvas reads as flicker (globalAlpha applies to
	// drawImage). When a blur is selected, render it even outside [start, end]:
	// float drift between a.start and t flickered fresh blurs on placement.
	// Export still honours start/end exactly.
	const isBlur = a.kind.kind === "blur";
	const isSelected = a.id === store.selectedAnnotationId;
	const editing = store.activePanel === "annotations";
	// The compositor draws every annotation inside its own window, so the only
	// thing left here is the ghost it cannot know about: the selected one being
	// moved while the playhead sits outside its window.
	if (compositorPaintsArtwork) {
		if (!isSelected || !editing || (t >= a.start && t <= a.end)) return;
	}
	// Outside its time window an annotation is invisible. Keep showing the
	// SELECTED one as a dim ghost while editing so moving/resizing it (its
	// handles draw regardless of time) doesn't make it vanish under the cursor.
	let renderOpacity = opacity;
	if (isBlur) {
		if (!isSelected && (t < a.start || t > a.end)) return;
	} else if (opacity <= 0) {
		if (isSelected && editing) renderOpacity = 0.35;
		else return;
	}
	if (!isCanvasDrawn(a.kind)) return; // text is rendered by TextAnnotationLayer

	if (a.kind.kind === "arrow") {
		const rArrow = rectFor(a);
		const p1 = projectA(a, a.kind.x1, a.kind.y1, t);
		const p2 = projectA(a, a.kind.x2, a.kind.y2, t);
		paintArrow(ctx, a, p1, p2, rArrow.w, renderOpacity);
		return;
	}

	const r = rectFor(a);
	const box = normaliseBox(a.kind);
	const topLeft = projectA(a, box.x, box.y, t);
	const bottomRight = projectA(a, box.x + box.w, box.y + box.h, t);
	const rect = {
		x: topLeft.x,
		y: topLeft.y,
		w: bottomRight.x - topLeft.x,
		h: bottomRight.y - topLeft.y,
	};
	if (rect.w <= 0 || rect.h <= 0) return;

	// Blur reads the WebGL composite (component-local); every other kind paints
	// through the shared @recast/render path so preview == export.
	if (a.kind.kind === "blur") {
		drawBlur(ctx, a, rect);
		return;
	}
	paintBoxAnnotation(ctx, a, rect, r.w, renderOpacity, {
		getImage: (p) => getImage(p),
		dpr: getDpr(),
	});
}

// Blur samples `compositeCanvasEl` (the WebGL frame); paintBlur is shared with
// the export, which feeds its own GL canvas + scratch through the same path.
function drawBlur(
	ctx: CanvasRenderingContext2D,
	a: Annotation,
	rect: { x: number; y: number; w: number; h: number },
) {
	if (a.kind.kind !== "blur" || !compositeCanvasEl) return;
	paintBlur(ctx, { opacity: a.opacity, kind: a.kind }, rect, {
		composite: compositeCanvasEl,
		srcW: compositeCanvasEl.width,
		srcH: compositeCanvasEl.height,
		dstW: canvasEl?.width ?? 0,
		dstH: canvasEl?.height ?? 0,
		getScratch: (w, h) => {
			const c = getBlurScratch(w, h);
			return c ? { ctx: c, canvas: blurScratch as CanvasImageSource } : null;
		},
	});
}

// Decoded <img> per source path, reused across frames. The rAF loop repaints
// continuously, so a load that finishes later shows up on the next frame.
type ImageEntry = {
	img: HTMLImageElement;
	ready: boolean;
	failed: boolean;
	failedAt: number;
};
const imageCache = new Map<string, ImageEntry>();
const IMAGE_RETRY_MS = 4000;

function getImage(path: string): ImageEntry {
	let entry = imageCache.get(path);
	// Retry a failed load after a delay so a restored/renamed file recovers
	// within the session instead of showing the placeholder forever.
	if (entry?.failed && Date.now() - entry.failedAt > IMAGE_RETRY_MS) {
		imageCache.delete(path);
		entry = undefined;
	}
	if (!entry) {
		const img = new Image();
		entry = { img, ready: false, failed: false, failedAt: 0 };
		const e = entry;
		img.onload = () => {
			e.ready = true;
		};
		img.onerror = () => {
			e.failed = true;
			e.failedAt = Date.now();
		};
		img.src = getEditorServices().resolveAssetUrl(path);
		imageCache.set(path, entry);
	}
	return entry;
}

// Evict cached bitmaps no longer referenced by any annotation, so replacing
// or deleting images doesn't accumulate decoded images for the editor's life.
$effect(() => {
	const live = new Set<string>();
	for (const a of store.annotations) {
		if (a.kind.kind === "image" && a.kind.path) live.add(a.kind.path);
	}
	const stale: string[] = [];
	for (const path of imageCache.keys()) {
		if (!live.has(path)) stale.push(path);
	}
	for (const path of stale) imageCache.delete(path);
});

// Reusable offscreen scratch that paintBlur renders the blur + tint into before
// compositing under a rounded clip (a rounded clip isn't honoured while the blur
// filter is active, so paintBlur applies corners in a filter-free second pass).
let blurScratch: HTMLCanvasElement | null = null;
function getBlurScratch(w: number, h: number): CanvasRenderingContext2D | null {
	if (!blurScratch) blurScratch = document.createElement("canvas");
	if (blurScratch.width !== w || blurScratch.height !== h) {
		blurScratch.width = w;
		blurScratch.height = h;
	}
	return blurScratch.getContext("2d");
}

/** A single resize grip: a rounded square with the surface fill, a crisp
 *  primary border and a soft drop shadow, matching the recording overlay's
 *  handle language. Shadow is applied to the fill only. */
function drawHandle(
	ctx: CanvasRenderingContext2D,
	cx: number,
	cy: number,
	dpr: number,
	palette: ReturnType<typeof selectionPalette>,
) {
	const hs = HANDLE_RADIUS_PX * dpr;
	const r = HANDLE_CORNER_PX * dpr;
	ctx.beginPath();
	roundRectPath(ctx, cx - hs, cy - hs, hs * 2, hs * 2, r);
	ctx.save();
	ctx.shadowColor = "rgba(0, 0, 0, 0.25)";
	ctx.shadowBlur = 3 * dpr;
	ctx.shadowOffsetY = 0.5 * dpr;
	ctx.fillStyle = palette.surface;
	ctx.fill();
	ctx.restore();
	ctx.lineWidth = 1.5 * dpr;
	ctx.strokeStyle = palette.accent;
	ctx.stroke();
}

function drawSelection(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
	const dpr = getDpr();
	const palette = selectionPalette();
	ctx.save();
	ctx.setLineDash([]);

	if (a.kind.kind === "arrow") {
		const p1 = projectA(a, a.kind.x1, a.kind.y1, t);
		const p2 = projectA(a, a.kind.x2, a.kind.y2, t);
		for (const pt of [p1, p2]) drawHandle(ctx, pt.x, pt.y, dpr, palette);
		ctx.restore();
		return;
	}

	const box = normaliseBox(a.kind);
	const topLeft = projectA(a, box.x, box.y, t);
	const bottomRight = projectA(a, box.x + box.w, box.y + box.h, t);
	const x = topLeft.x;
	const y = topLeft.y;
	const w = bottomRight.x - topLeft.x;
	const h = bottomRight.y - topLeft.y;

	// Soft outer ring then the crisp primary border, mirroring the recording
	// area selection's `border-primary ring-primary/40`.
	ctx.strokeStyle = palette.accentRing;
	ctx.lineWidth = 3 * dpr;
	ctx.strokeRect(x, y, w, h);
	ctx.strokeStyle = palette.accent;
	ctx.lineWidth = 1.5 * dpr;
	ctx.strokeRect(x, y, w, h);

	const handles = handlePositions(x, y, w, h);
	for (const [, pt] of Object.entries(handles)) {
		drawHandle(ctx, pt.x, pt.y, dpr, palette);
	}
	ctx.restore();
}

/** Dimension badge shown while placing or resizing, in output-video pixels.
 *  Centred below the box, as Figma and Framer both place it; above-left put it
 *  directly under the pointer on a north-west drag. Flips above when there is
 *  no room below. */
function drawSizeBadge(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
	if (a.kind.kind === "arrow" || !store.metadata || !canvasEl) return;
	const dpr = getDpr();
	const palette = selectionPalette();
	const box = normaliseBox(a.kind);
	const tl = projectA(a, box.x, box.y, t);
	const br = projectA(a, box.x + box.w, box.y + box.h, t);
	const wPx = Math.round(box.w * store.metadata.width);
	const hPx = Math.round(box.h * store.metadata.height);
	const label = `${wPx} × ${hPx}`;

	ctx.save();
	ctx.font = `600 ${11 * dpr}px ${palette.monoFamily}`;
	ctx.textBaseline = "middle";
	const padX = 6 * dpr;
	const chipH = 18 * dpr;
	const gap = 6 * dpr;
	const textW = ctx.measureText(label).width;
	const chipW = textW + padX * 2;
	const chipX = Math.min(
		Math.max((tl.x + br.x) / 2 - chipW / 2, gap),
		Math.max(gap, canvasEl.width - chipW - gap),
	);
	const below = br.y + gap;
	const chipY = below + chipH + gap <= canvasEl.height ? below : Math.max(tl.y - chipH - gap, gap);

	ctx.beginPath();
	roundRectPath(ctx, chipX, chipY, chipW, chipH, 3 * dpr);
	ctx.fillStyle = palette.accent;
	ctx.fill();
	ctx.fillStyle = palette.onAccent;
	ctx.fillText(label, chipX + padX, chipY + chipH / 2 + 0.5 * dpr);
	ctx.restore();
}

/** Hover-flash from the layer panel: pulse a 2px outline around the shape. */
function drawHoverFlash(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
	const dpr = getDpr();
	ctx.save();
	ctx.strokeStyle = selectionPalette().accentMuted;
	ctx.lineWidth = 2 * dpr;
	ctx.setLineDash([]);

	if (a.kind.kind === "arrow") {
		const p1 = projectA(a, a.kind.x1, a.kind.y1, t);
		const p2 = projectA(a, a.kind.x2, a.kind.y2, t);
		ctx.beginPath();
		ctx.moveTo(p1.x, p1.y);
		ctx.lineTo(p2.x, p2.y);
		ctx.stroke();
		ctx.restore();
		return;
	}

	const box = normaliseBox(a.kind);
	const tl = projectA(a, box.x, box.y, t);
	const br = projectA(a, box.x + box.w, box.y + box.h, t);
	const pad = 4 * dpr;
	ctx.strokeRect(tl.x - pad, tl.y - pad, br.x - tl.x + pad * 2, br.y - tl.y + pad * 2);
	ctx.restore();
}

//  Frame loop

// Whether the last draw put anything on the canvas. A `clearRect` over a
// full-viewport DPR layer dirties it for the compositor every frame even when
// there is nothing to draw — which is the common case.
let paintedLastFrame = false;

function draw() {
	if (!canvasEl || !store.metadata) return;

	const ordered = store.annotationsGloballyHidden ? [] : store.annotationsByZ;
	const willPaint = ordered.length > 0;
	// Still runs the frame that goes empty, so the last annotation is cleared.
	if (!willPaint && !paintedLastFrame) return;
	paintedLastFrame = willPaint;

	resizeToContainer();
	const ctx = canvasEl.getContext("2d");
	if (!ctx) return;

	ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

	if (store.annotationsGloballyHidden) return;

	const t = playbackTime();
	for (const a of ordered) {
		if (a.hidden) continue;
		const opacity = evalOpacity(a, t);
		drawAnnotation(ctx, a, opacity, t);
	}

	// Selection adornment + hover-flash only show on the Annotations tab so
	// the editing handles don't clutter the preview while the user is on
	// other panels.
	if (store.activePanel === "annotations") {
		const hover =
			store.hoveredAnnotationId && store.hoveredAnnotationId !== store.selectedAnnotationId
				? store.annotations.find((a) => a.id === store.hoveredAnnotationId)
				: null;
		if (hover && !hover.hidden) drawHoverFlash(ctx, hover, t);

		const sel = store.annotations.find((a) => a.id === store.selectedAnnotationId);
		if (sel && !sel.hidden) {
			drawSelection(ctx, sel, t);
			// Live dimensions while actively sizing (place/resize), not on idle
			// selection or plain moves, so the chip appears only when it helps.
			if (drag && (drag.kind === "place" || drag.kind === "resize") && drag.id === sel.id) {
				drawSizeBadge(ctx, sel, t);
			}
		}

		if (snapGuides.length > 0) drawSnapGuides(ctx, t);
	}
}

/** Draw the snap guides emitted during the active drag. Two guides max in
 *  practice (one per axis); the cap in `applySnap` enforces a hard ceiling. */
function drawSnapGuides(ctx: CanvasRenderingContext2D, t: number) {
	const dpr = getDpr();
	// Guides live in the dragged annotation's space so they line up with it.
	const activeDrag = drag;
	const anchorObj = (activeDrag && store.annotations.find((x) => x.id === activeDrag.id)) || {};
	const r = rectFor(anchorObj);
	const zoom = zoomFor(anchorObj, t);
	if (r.w <= 0 || r.h <= 0) return;

	ctx.save();
	ctx.strokeStyle = selectionPalette().accentMuted;
	ctx.lineWidth = 1 * dpr;
	ctx.setLineDash([4 * dpr, 3 * dpr]);

	for (const g of snapGuides) {
		if (g.axis === "x") {
			const top = uvToCanvas(g.value, 0, r, zoom);
			const bot = uvToCanvas(g.value, 1, r, zoom);
			ctx.beginPath();
			ctx.moveTo(top.x, top.y);
			ctx.lineTo(bot.x, bot.y);
			ctx.stroke();
		} else {
			const left = uvToCanvas(0, g.value, r, zoom);
			const right = uvToCanvas(1, g.value, r, zoom);
			ctx.beginPath();
			ctx.moveTo(left.x, left.y);
			ctx.lineTo(right.x, right.y);
			ctx.stroke();
		}
	}
	ctx.restore();
}

/** Only the moving picture and an active drag need a frame-by-frame repaint. */
function needsContinuousRedraw(): boolean {
	return store.isPlaying || drag !== null;
}

function tick() {
	rafHandle = null;
	draw();
	if (needsContinuousRedraw()) scheduleRedraw();
}

/**
 * Coalesced one-shot repaint. This loop used to re-arm unconditionally for the
 * whole editor session, so even with zero annotations it cleared a full
 * DPR-scaled canvas 60x/sec — a full-viewport layer the compositor had to
 * re-upload every frame, forever.
 */
function scheduleRedraw() {
	if (rafHandle !== null) return;
	rafHandle = requestAnimationFrame(tick);
}

function resizeToContainer() {
	if (!canvasEl) return;
	// Fallback: if the cached size is still unknown (target not yet laid out
	// when the observer was set up), measure live so the canvas never gets
	// stuck at 1x1. A 1x1 backing stretched over the preview renders the first
	// near-white handle as a full-screen white wash.
	if ((targetSize.w <= 0 || targetSize.h <= 0) && targetEl) {
		const r = targetEl.getBoundingClientRect();
		targetSize = { w: r.width, h: r.height };
	}
	const dpr = getDpr();
	const w = Math.max(1, Math.floor(targetSize.w * dpr));
	const h = Math.max(1, Math.floor(targetSize.h * dpr));
	if (canvasEl.width !== w || canvasEl.height !== h) {
		canvasEl.width = w;
		canvasEl.height = h;
	}
}

//  Pointer interaction

function pickAnnotation(pt: { x: number; y: number }, t: number) {
	const dpr = getDpr();
	return hitTestAnnotation(pt, store.annotationsByZ, {
		project: (a, ux, uy) => projectA(a, ux, uy, t),
		t,
		handleSlop: HANDLE_RADIUS_PX * dpr + 2 * dpr,
		lineSlop: 6 * dpr,
		annotationSlop: 8 * dpr,
	});
}

function pickHandle(pt: { x: number; y: number }, a: Annotation, t: number) {
	const dpr = getDpr();
	return hitTestHandle(pt, a, {
		project: (anno, ux, uy) => projectA(anno, ux, uy, t),
		t,
		handleSlop: HANDLE_RADIUS_PX * dpr + 2 * dpr,
		lineSlop: 6 * dpr,
		annotationSlop: 8 * dpr,
	});
}

function handlePointerDown(e: PointerEvent) {
	scheduleRedraw();
	if (!canvasEl || !store.metadata) return;
	if (store.annotationsGloballyHidden) return;
	const pt = pointerToCanvasPx(e);
	const t = playbackTime();

	// Selected annotation's handles come first (so you can resize over top of others).
	const selected = store.annotations.find((a) => a.id === store.selectedAnnotationId);
	if (selected && !selected.locked && !selected.hidden) {
		const hit = pickHandle(pt, selected, t);
		if (hit && hit !== "body") {
			(e.currentTarget as Element).setPointerCapture(e.pointerId);
			const box = normaliseBox(selected.kind);
			drag = { kind: "resize", id: selected.id, handle: hit, startBox: box };
			dragUndoPushed = false;
			e.preventDefault();
			return;
		}
		if (hit === "body") {
			// Body of the already-selected annotation → start moving immediately.
			// We deliberately skip the pickAnnotation path here so the annotation
			// can be moved during fade-in / fade-out windows where evalOpacity
			// would otherwise filter it out of the hit-test.
			(e.currentTarget as Element).setPointerCapture(e.pointerId);
			const pointerUV = unprojectA(selected, pt.x, pt.y, t);
			if (selected.kind.kind === "arrow") {
				drag = {
					kind: "move",
					id: selected.id,
					startX: selected.kind.x1,
					startY: selected.kind.y1,
					startX2: selected.kind.x2,
					startY2: selected.kind.y2,
					pointerStartUV: pointerUV,
				};
			} else {
				const box = normaliseBox(selected.kind);
				drag = {
					kind: "move",
					id: selected.id,
					startX: box.x,
					startY: box.y,
					pointerStartUV: pointerUV,
				};
			}
			dragUndoPushed = false;
			e.preventDefault();
			return;
		}
	}

	// Any annotation under the pointer → select and enter move mode.
	const hitAnno = pickAnnotation(pt, t);
	if (hitAnno) {
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		store.selectedAnnotationId = hitAnno.id;
		const pointerUV = unprojectA(hitAnno, pt.x, pt.y, t);
		if (hitAnno.kind.kind === "arrow") {
			drag = {
				kind: "move",
				id: hitAnno.id,
				startX: hitAnno.kind.x1,
				startY: hitAnno.kind.y1,
				startX2: hitAnno.kind.x2,
				startY2: hitAnno.kind.y2,
				pointerStartUV: pointerUV,
			};
		} else {
			const box = normaliseBox(hitAnno.kind);
			drag = {
				kind: "move",
				id: hitAnno.id,
				startX: box.x,
				startY: box.y,
				pointerStartUV: pointerUV,
			};
		}
		dragUndoPushed = false;
		e.preventDefault();
		return;
	}

	// No hit. If a tool is active, start placing a new annotation.
	const tool = store.annotationTool;
	if (tool) {
		// New annotations default to the video anchor.
		const anchor = unprojectA({}, pt.x, pt.y, t);
		let kind: AnnotationKind;
		switch (tool) {
			case "rect":
				kind = { kind: "rect", x: anchor.x, y: anchor.y, w: 0, h: 0, radius: 0.005 };
				break;
			case "ellipse":
				kind = { kind: "ellipse", x: anchor.x, y: anchor.y, w: 0, h: 0 };
				break;
			case "arrow":
				kind = {
					kind: "arrow",
					x1: anchor.x,
					y1: anchor.y,
					x2: anchor.x,
					y2: anchor.y,
					headSize: 0.15,
				};
				break;
			case "text":
				kind = {
					kind: "text",
					x: anchor.x,
					y: anchor.y,
					w: 0,
					h: 0,
					content: "Type here",
					fontFamily: "'Geist Variable', system-ui, sans-serif",
					fontSize: 0.06,
					fontWeight: 600,
					color: "#ffffff",
					align: "left",
					lineHeight: 1.2,
				};
				break;
			case "blur":
				kind = {
					kind: "blur",
					x: anchor.x,
					y: anchor.y,
					w: 0,
					h: 0,
					strength: 0.5,
					variant: "glass",
					tintColor: "#000000",
					radius: 0.005,
				};
				break;
			// "image" is a one-shot insert from the panel, never an armed tool.
			default:
				return;
		}
		const placed = store.addAnnotation(kind);
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		drag = { kind: "place", id: placed.id, anchor };
		e.preventDefault();
		return;
	}

	// Otherwise: deselect.
	store.selectedAnnotationId = null;
}

function applySnap(
	ux: number,
	uy: number,
	dragId: string | null,
	altHeld: boolean,
): { x: number; y: number } {
	if (altHeld || !store.annotationSnapEnabled) {
		snapGuides = [];
		return { x: ux, y: uy };
	}
	const anchors = buildAnnotationSnapAnchors(store.annotations, dragId);
	const result = snap(ux, uy, anchors, 0.005, true);
	// Cap to 4 simultaneous guides (one per axis is the typical case; never
	// more than 2 from this fn, but keep the cap for safety).
	snapGuides = result.guides.slice(0, 4);
	return { x: result.x, y: result.y };
}

/** Refresh the hover state used for cursor affordance. Runs only when no
 *  drag is in flight so the cursor flips between grab/resize as the user
 *  passes over annotations. */
function refreshHover(pt: { x: number; y: number }, t: number) {
	if (drag) return;
	if (store.annotationTool) {
		hoverHandle = "tool";
		return;
	}
	const selected = store.annotations.find((a) => a.id === store.selectedAnnotationId);
	if (selected && !selected.locked && !selected.hidden) {
		const handle = pickHandle(pt, selected, t);
		if (handle && handle !== "body") {
			hoverHandle = handle;
			return;
		}
	}
	const hit = pickAnnotation(pt, t);
	hoverHandle = hit ? "body" : null;
}

function frameDims(): { w: number; h: number } {
	return { w: store.metadata?.width ?? 16, h: store.metadata?.height ?? 9 };
}

function handlePointerMove(e: PointerEvent) {
	scheduleRedraw();
	if (!drag) {
		refreshHover(pointerToCanvasPx(e), playbackTime());
		return;
	}
	const pt = pointerToCanvasPx(e);
	const t = playbackTime();
	const f = frameDims();
	const dragAnno = store.annotations.find((x) => x.id === drag!.id) ?? {};
	const rawUv = unprojectA(dragAnno, pt.x, pt.y, t);
	// Alt held bypasses snap, matching Figma. Snap is per-axis so an annotation
	// can lock to a horizontal guide while still tracking the cursor vertically.
	const uv = applySnap(rawUv.x, rawUv.y, drag.id, e.altKey);

	// First real move of a move/resize commits one undo entry (placement
	// pushed at creation).
	if (!dragUndoPushed) {
		store.pushUndoState();
		dragUndoPushed = true;
	}

	if (drag.kind === "place") {
		const anno = store.annotations.find((a) => a.id === drag!.id);
		if (!anno) return;
		if (anno.kind.kind === "arrow") {
			const end = e.shiftKey
				? constrain45(anno.kind.x1, anno.kind.y1, uv.x, uv.y, f.w, f.h)
				: { x: uv.x, y: uv.y };
			store.updateAnnotation(drag.id, {
				kind: { ...anno.kind, x2: end.x, y2: end.y },
			});
		} else if (
			anno.kind.kind === "rect" ||
			anno.kind.kind === "ellipse" ||
			anno.kind.kind === "text" ||
			anno.kind.kind === "image" ||
			anno.kind.kind === "blur"
		) {
			let w = uv.x - drag.anchor.x;
			let h = uv.y - drag.anchor.y;
			if (e.shiftKey) ({ w, h } = constrainSquare(w, h, f.w, f.h));
			store.updateAnnotation(drag.id, {
				kind: { ...anno.kind, x: drag.anchor.x, y: drag.anchor.y, w, h },
			});
		}
	} else if (drag.kind === "move") {
		const anno = store.annotations.find((a) => a.id === drag!.id);
		if (!anno) return;
		if (anno.kind.kind === "arrow") {
			const dx = uv.x - drag.pointerStartUV.x;
			const dy = uv.y - drag.pointerStartUV.y;
			const sx2 = drag.startX2 ?? anno.kind.x2;
			const sy2 = drag.startY2 ?? anno.kind.y2;
			store.updateAnnotation(drag.id, {
				kind: {
					...anno.kind,
					x1: drag.startX + dx,
					y1: drag.startY + dy,
					x2: sx2 + dx,
					y2: sy2 + dy,
				},
			});
		} else if (
			anno.kind.kind === "rect" ||
			anno.kind.kind === "ellipse" ||
			anno.kind.kind === "text" ||
			anno.kind.kind === "image" ||
			anno.kind.kind === "blur"
		) {
			// Snap the box's own edges/center to guides (not the raw cursor), so a
			// move aligns the annotation itself. Alt or the snap toggle bypasses.
			const rawDx = rawUv.x - drag.pointerStartUV.x;
			const rawDy = rawUv.y - drag.pointerStartUV.y;
			const bx = drag.startX + rawDx;
			const by = drag.startY + rawDy;
			const b = normaliseBox(anno.kind);
			let newX = bx;
			let newY = by;
			if (!e.altKey && store.annotationSnapEnabled) {
				const res = snapBox(
					bx,
					by,
					b.w,
					b.h,
					buildAnnotationSnapAnchors(store.annotations, drag.id),
					0.005,
				);
				newX = res.x;
				newY = res.y;
				snapGuides = res.guides.slice(0, 4);
			} else {
				snapGuides = [];
			}
			store.updateAnnotation(drag.id, {
				kind: { ...anno.kind, x: newX, y: newY },
			});
		}
	} else if (drag.kind === "resize") {
		const anno = store.annotations.find((a) => a.id === drag!.id);
		if (!anno) return;
		if (anno.kind.kind === "arrow") {
			if (drag.handle === "p1") {
				const p = e.shiftKey
					? constrain45(anno.kind.x2, anno.kind.y2, uv.x, uv.y, f.w, f.h)
					: { x: uv.x, y: uv.y };
				store.updateAnnotation(drag.id, {
					kind: { ...anno.kind, x1: p.x, y1: p.y },
				});
			} else if (drag.handle === "p2") {
				const p = e.shiftKey
					? constrain45(anno.kind.x1, anno.kind.y1, uv.x, uv.y, f.w, f.h)
					: { x: uv.x, y: uv.y };
				store.updateAnnotation(drag.id, {
					kind: { ...anno.kind, x2: p.x, y2: p.y },
				});
			}
			return;
		}

		const b = drag.startBox;
		let nx = b.x;
		let ny = b.y;
		let nw = b.w;
		let nh = b.h;
		const h = drag.handle;
		if (h === "nw" || h === "w" || h === "sw") {
			nw = b.w + (b.x - uv.x);
			nx = uv.x;
		}
		if (h === "ne" || h === "e" || h === "se") {
			nw = uv.x - b.x;
		}
		if (h === "nw" || h === "n" || h === "ne") {
			nh = b.h + (b.y - uv.y);
			ny = uv.y;
		}
		if (h === "sw" || h === "s" || h === "se") {
			nh = uv.y - b.y;
		}
		// Shift on a corner locks to the starting aspect ratio.
		if (e.shiftKey && isCornerHandle(h)) {
			({ nx, ny, nw, nh } = lockAspect(h, b, nx, ny, nw, nh));
		}
		if (
			anno.kind.kind === "rect" ||
			anno.kind.kind === "ellipse" ||
			anno.kind.kind === "text" ||
			anno.kind.kind === "image" ||
			anno.kind.kind === "blur"
		) {
			store.updateAnnotation(drag.id, {
				kind: { ...anno.kind, x: nx, y: ny, w: nw, h: nh },
			});
		}
	}
}

function handlePointerUp(e: PointerEvent) {
	scheduleRedraw();
	if (!drag) return;
	(e.currentTarget as Element).releasePointerCapture(e.pointerId);
	// Drop snap guides immediately on release so the preview returns to
	// a clean state on click (no lingering guides between drags).
	snapGuides = [];
	if (drag.kind === "place") {
		const anno = store.annotations.find((a) => a.id === drag!.id);
		const f = frameDims();
		if (anno) {
			if (
				anno.kind.kind === "rect" ||
				anno.kind.kind === "ellipse" ||
				anno.kind.kind === "image" ||
				anno.kind.kind === "blur"
			) {
				if (Math.abs(anno.kind.w) < 0.01 || Math.abs(anno.kind.h) < 0.01) {
					// A click with no drag places a default-sized shape rather than
					// cancelling. Text already worked this way; arming the Rectangle
					// tool and clicking used to produce nothing at all.
					const box = clickPlacedBox(drag.anchor.x, drag.anchor.y, f.w, f.h);
					store.updateAnnotation(drag.id, { kind: { ...anno.kind, ...box } });
					store.selectedAnnotationId = drag.id;
				}
			} else if (anno.kind.kind === "text") {
				if (Math.abs(anno.kind.w) < 0.04) {
					store.updateAnnotation(drag.id, {
						kind: { ...anno.kind, w: 0.25 },
					});
				}
				if (Math.abs(anno.kind.h) < 0.04) {
					store.updateAnnotation(drag.id, {
						kind: { ...anno.kind, h: anno.kind.fontSize * 1.6 },
					});
				}
			} else if (anno.kind.kind === "arrow") {
				const dx = anno.kind.x2 - anno.kind.x1;
				const dy = anno.kind.y2 - anno.kind.y1;
				if (Math.hypot(dx, dy) < 0.01) {
					const pts = clickPlacedArrow(anno.kind.x1, anno.kind.y1, f.w, f.h);
					store.updateAnnotation(drag.id, { kind: { ...anno.kind, ...pts } });
					store.selectedAnnotationId = drag.id;
				}
			}
		}
		// After placement, drop the tool so the user doesn't create stacked
		// shapes on their next click. Matches Figma/Keynote behaviour.
		store.annotationTool = null;
	} else if (drag.kind === "resize" || drag.kind === "move") {
		const anno = store.annotations.find((a) => a.id === drag!.id);
		if (
			anno &&
			(anno.kind.kind === "rect" ||
				anno.kind.kind === "ellipse" ||
				anno.kind.kind === "text" ||
				anno.kind.kind === "image" ||
				anno.kind.kind === "blur")
		) {
			const box = normaliseBox(anno.kind);
			store.updateAnnotation(drag.id, {
				kind: { ...anno.kind, x: box.x, y: box.y, w: box.w, h: box.h },
			});
		}
	}
	drag = null;
}

function nudgeBy(dxUV: number, dyUV: number) {
	const id = store.selectedAnnotationId;
	if (!id) return;
	const a = store.annotations.find((x) => x.id === id);
	if (!a || a.locked || a.hidden) return;
	if (a.kind.kind === "arrow") {
		store.updateAnnotation(id, {
			kind: {
				...a.kind,
				x1: a.kind.x1 + dxUV,
				y1: a.kind.y1 + dyUV,
				x2: a.kind.x2 + dxUV,
				y2: a.kind.y2 + dyUV,
			},
		});
	} else if (
		a.kind.kind === "rect" ||
		a.kind.kind === "ellipse" ||
		a.kind.kind === "text" ||
		a.kind.kind === "image" ||
		a.kind.kind === "blur"
	) {
		store.updateAnnotation(id, {
			kind: { ...a.kind, x: a.kind.x + dxUV, y: a.kind.y + dyUV },
		});
	}
}

function handleKeyDown(e: KeyboardEvent) {
	// Typing surfaces own every key. Escape especially: TextAnnotationLayer uses
	// it to cancel an inline text edit, and clearing the selection here too meant
	// one Escape both reverted the text AND closed the panel that was editing it.
	if (isEditableTarget(e.target)) return;
	if (e.key === "Escape") {
		if (store.annotationTool) {
			store.annotationTool = null;
			e.preventDefault();
		} else if (store.selectedAnnotationId) {
			store.selectedAnnotationId = null;
			e.preventDefault();
		}
		return;
	}
	// Delete is owned by the editor page and acts on the current selection, so it
	// is deliberately not handled here. This listener is on `window`, so claiming
	// Delete meant a clip-scoped Delete elsewhere could ALSO delete a selected
	// annotation on the same keypress.

	// Z-order shortcuts and duplicate, gated to annotations tab + selection
	// so they don't fight other editor surfaces.
	if (
		store.activePanel === "annotations" &&
		store.selectedAnnotationId &&
		(e.metaKey || e.ctrlKey) &&
		!e.altKey
	) {
		if (e.key === "]") {
			e.preventDefault();
			store.reorderAnnotation(store.selectedAnnotationId, 1);
			return;
		}
		if (e.key === "[") {
			e.preventDefault();
			store.reorderAnnotation(store.selectedAnnotationId, -1);
			return;
		}
		if (e.key.toLowerCase() === "d" && !e.shiftKey) {
			e.preventDefault();
			store.duplicateAnnotation(store.selectedAnnotationId);
			return;
		}
	}

	// Alt+arrow, never a bare arrow — see nudgeVectorPx.
	const nudge = nudgeVectorPx(e);
	if (nudge && store.activePanel === "annotations" && store.selectedAnnotationId) {
		const selForNudge = store.annotations.find((x) => x.id === store.selectedAnnotationId) ?? {};
		const r = rectFor(selForNudge);
		if (r.w <= 0 || r.h <= 0) return;
		// Device pixels → UV, against the rect this annotation is anchored to.
		const dx = nudge.dx / Math.max(1, r.w);
		const dy = nudge.dy / Math.max(1, r.h);
		e.preventDefault();
		// Coalesce a held/repeated arrow key into one undo entry (same key the
		// timeline layer card uses), so Ctrl+Z reverts the nudge, not an
		// unrelated earlier edit.
		store.pushUndoStateCoalesced(`nudge-annotation-${store.selectedAnnotationId}`, 600);
		nudgeBy(dx, dy);
	}
}

//  Lifecycle

// Track the container size. A $effect (not onMount) so it re-establishes if
// `targetEl` arrives after mount, and getBoundingClientRect (rendered size)
// so a scaled/letterboxed preview maps to the right backing resolution.
$effect(() => {
	const el = targetEl;
	if (!el) return;
	const measure = () => {
		const r = el.getBoundingClientRect();
		if (r.width > 0 && r.height > 0) targetSize = { w: r.width, h: r.height };
	};
	measure();
	const ro = new ResizeObserver(() => {
		measure();
		scheduleRedraw();
	});
	ro.observe(el);
	return () => ro.disconnect();
});

// Repaint on the state the drawing actually depends on. `targetSize` is a plain
// local, so its call sites schedule directly.
$effect(() => {
	void store.annotationsByZ;
	void previewTime;
	void store.currentTime;
	void store.selectedAnnotationId;
	void store.hoveredAnnotationId;
	void store.activePanel;
	void store.annotationsGloballyHidden;
	void store.annotationTool;
	void snapGuides;
	void store.isPlaying;
	// Projection inputs: without these, changing padding, aspect or a zoom
	// region while paused left the markup drawn against the previous geometry
	// until something else happened to trigger a frame.
	void store.padding;
	void store.outputAspect;
	void store.metadata;
	void store.zoomRegions;
	void store.focusEnabled;
	scheduleRedraw();
});

onMount(() => {
	scheduleRedraw();
});

onDestroy(() => {
	if (rafHandle !== null) cancelAnimationFrame(rafHandle);
	disposeCanvasTokens();
});

// Editing is annotations-tab-only everywhere else in this file (selection chrome,
// nudge, z-order), and TextAnnotationLayer gates its own pointer-events the same
// way. The canvas didn't, so a click on the Audio or Captions tab could select
// and drag a shape with no handles drawn to show it had happened.
const interactive = $derived(
	store.activePanel === "annotations" && !store.annotationsGloballyHidden,
);

const canvasCursor = $derived.by(() => {
	if (store.annotationTool) return "crosshair";
	if (drag?.kind === "move") return "grabbing";
	if (drag?.kind === "resize") return cursorForHandle(drag.handle);
	return cursorForHandle(hoverHandle);
});
</script>

<!-- Local annotation editing keys (delete, deselect, Mod+D/[/], arrow nudge,
     documented in the central shortcut registry). `<svelte:window>` so HMR
     rebinds rather than leaks the listener. -->
<svelte:window onkeydown={handleKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<canvas
  bind:this={canvasEl}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  onpointerleave={() => (hoverHandle = null)}
  class="absolute inset-0 h-full w-full"
  style:pointer-events={interactive ? "auto" : "none"}
  style:touch-action="none"
  style:cursor={canvasCursor}
></canvas>
