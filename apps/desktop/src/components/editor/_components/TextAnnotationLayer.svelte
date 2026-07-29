<script lang="ts">
import { evalOpacity, evalZoom } from "$lib/annotations/eval";
import { ensureFontLoaded } from "$lib/fonts/font-options";
import { canvasToUV, compositionRectPx, uvToCanvas, videoRectPx } from "$lib/annotations/uv";
import { snap } from "$lib/annotations/snap";
import { IDENTITY_ZOOM, withAlpha } from "./annotation-draw.logic";
import { buildAnnotationSnapAnchors } from "./annotation-snap.logic";
import type { Annotation, AnnotationAnchor, EditorStore } from "$lib/stores/editor-store.svelte";
import { onDestroy, onMount, tick } from "svelte";

// HTML layer (sibling to the 2D AnnotationOverlay) so text gets the WebView's
// full glyph rendering and contenteditable inline editing.
// PARITY: export rasterizes each text annotation to a PNG (lib/export/rasterize-text.ts);
// Rust never sees fonts.

interface Props {
	store: EditorStore;
	videoEl: HTMLVideoElement | null;
	/** The container that wraps the WebGL preview canvas, which we stretch to fit. */
	targetEl: HTMLElement | null;
}

let { store, videoEl, targetEl }: Props = $props();

// Fetch + register any Google fonts used by text annotations so they render
// in preview (and are available before export rasterizes the text).
$effect(() => {
	for (const a of store.annotations) {
		if (a.kind.kind === "text") ensureFontLoaded(a.kind.fontFamily, a.kind.fontWeight);
	}
});

let layerEl: HTMLDivElement | undefined = $state();
let layerSize = $state({ w: 0, h: 0 });
let editingId = $state<string | null>(null);
// Pre-edit text, captured on entry so Escape can restore it.
let editStartContent = "";
let rafHandle: number | null = null;
// rAF tick to rebuild positions per frame (store doesn't fire on every video tick).
let _frame = $state(0);

// Own pointer flow (text is a sibling HTML element), using the same UV math + snap engine as the canvas.
type TextDrag = {
	id: string;
	startX: number; // UV
	startY: number;
	pointerStartUV: { x: number; y: number };
	moved: boolean; // true once we cross the click vs drag threshold
} | null;
let drag: TextDrag = $state(null);
// Below this (CSS px) the gesture is a click (select); above it, a move.
const CLICK_DRAG_THRESHOLD_PX = 3;

function rectCssFor(a: { anchor?: AnnotationAnchor }) {
	return a.anchor === "frame"
		? compositionRectPx(layerSize.w, layerSize.h, store.metadata, store.padding, store.outputAspect)
		: videoRectPx(layerSize.w, layerSize.h, store.metadata, store.padding, store.outputAspect);
}

function zoomForA(a: { anchor?: AnnotationAnchor }, t: number) {
	return a.anchor === "frame" ? IDENTITY_ZOOM : evalZoom(store.zoomRegions, t);
}

function uvToCss(a: { anchor?: AnnotationAnchor }, ux: number, uy: number, t: number) {
	return uvToCanvas(ux, uy, rectCssFor(a), zoomForA(a, t));
}

function pointerToUV(a: { anchor?: AnnotationAnchor }, e: PointerEvent, t: number) {
	if (!layerEl) return { x: 0, y: 0 };
	const rect = layerEl.getBoundingClientRect();
	return canvasToUV(e.clientX - rect.left, e.clientY - rect.top, rectCssFor(a), zoomForA(a, t));
}

function pointerToCss(e: PointerEvent) {
	if (!layerEl) return { x: 0, y: 0 };
	const rect = layerEl.getBoundingClientRect();
	return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function playbackTime(): number {
	return videoEl?.currentTime ?? store.currentTime;
}

function tick_() {
	rafHandle = null;
	// Fallback only while the size is still unknown; the $effect below keeps it
	// live once the element is observed.
	if ((layerSize.w <= 0 || layerSize.h <= 0) && layerEl) measureLayer();
	_frame++;
	// Only the moving picture needs a per-frame re-derive; `styleFor` reads
	// `videoEl.currentTime`, which is invisible to the reactive graph.
	if (store.isPlaying) scheduleTick();
}

/**
 * Coalesced re-derive. This used to re-arm unconditionally, dirtying `$state`
 * 60x/sec for the whole session and recomputing every text annotation's inline
 * style even when nothing was on screen.
 */
function scheduleTick() {
	if (rafHandle !== null) return;
	rafHandle = requestAnimationFrame(tick_);
}

function measureLayer() {
	if (!layerEl) return;
	const r = layerEl.getBoundingClientRect();
	layerSize = { w: r.width, h: r.height };
}

// Track the layer size with a ResizeObserver. A $effect (not onMount) so it
// re-attaches when `layerEl` binds. `targetEl` (the parent's bind:this) is
// still null at this child's onMount, which is why the old onMount observer
// never attached and `layerSize` froze, drifting the text off its selection
// box on any preview resize. Observe the always-present local `layerEl`.
$effect(() => {
	const el = layerEl;
	if (!el) return;
	measureLayer();
	const ro = new ResizeObserver(() => {
		measureLayer();
		scheduleTick();
	});
	ro.observe(el);
	return () => ro.disconnect();
});

// Re-derive positions when the state they depend on moves. While paused this
// replaces the old per-frame tick entirely.
$effect(() => {
	void store.currentTime;
	void store.annotations;
	void store.zoomRegions;
	void store.selectedAnnotationId;
	void store.isPlaying;
	void drag;
	void layerSize;
	scheduleTick();
});

onMount(() => {
	scheduleTick();
});
onDestroy(() => {
	if (rafHandle !== null) cancelAnimationFrame(rafHandle);
});

// `_frame` dependency forces re-derive on rAF ticks so position tracks playback/zoom.
function styleFor(a: Annotation): string {
	if (a.kind.kind !== "text") return "";
	void _frame;
	const t = playbackTime();
	const opacity = evalOpacity(a, t);
	const k = a.kind;
	const x = Math.min(k.x, k.x + k.w);
	const y = Math.min(k.y, k.y + k.h);
	const w = Math.abs(k.w);
	const h = Math.abs(k.h);
	const tl = uvToCss(a, x, y, t);
	const br = uvToCss(a, x + w, y + h, t);
	const cssW = Math.max(0, br.x - tl.x);
	const cssH = Math.max(0, br.y - tl.y);
	// Size the font and glow off the annotation's ANCHOR rect, not the full
	// layer. The export rasterizes at comp resolution then scales the raster
	// into the anchor rect (video rect for video-anchored, comp rect for
	// frame-anchored), so a video-anchored text shrinks with the video whenever
	// there's padding. Using layerSize.h/.w here made preview font/wrap comp-
	// relative and drifted off the exported glyphs. `glow.blur × rect.w` mirrors
	// draw_image_shadow's `glow.blur × (uv_to_canvas(1)−uv_to_canvas(0))`.
	const rect = rectCssFor(a);
	const fontSizePx = k.fontSize * rect.h;
	const z = a.zIndex ?? 0;
	// Glow → CSS drop-shadow so the preview matches the exported text (which
	// rasterizes to an image and picks up the same glow via draw_image_shadow).
	const g = a.glow;
	const glowFilter = g
		? `filter: drop-shadow(0 0 ${Math.max(0, g.blur * rect.w).toFixed(2)}px ${withAlpha(g.color, g.opacity)})`
		: "";
	return [
		`left: ${tl.x}px`,
		`top: ${tl.y}px`,
		`width: ${cssW}px`,
		`min-height: ${cssH}px`,
		`opacity: ${opacity}`,
		`z-index: ${z}`,
		`font-family: ${k.fontFamily}`,
		`font-size: ${fontSizePx}px`,
		`font-weight: ${k.fontWeight}`,
		`color: ${k.color}`,
		`text-align: ${k.align}`,
		`line-height: ${k.lineHeight}`,
		glowFilter,
	]
		.filter(Boolean)
		.join(";");
}

function startEditing(a: Annotation) {
	if (a.kind.kind !== "text") return;
	if (a.locked) return;
	// Remember the pre-edit text so Escape can cancel, and defer undo to commit
	// so a select/enter that changes nothing doesn't push a no-op entry.
	editStartContent = a.kind.content;
	editingId = a.id;
	void tick().then(() => {
		const el = document.querySelector(`[data-text-anno-id="${a.id}"]`) as HTMLElement | null;
		if (el) {
			el.focus();
			// Select all on entry. Keynote behaviour.
			const range = document.createRange();
			range.selectNodeContents(el);
			const sel = window.getSelection();
			sel?.removeAllRanges();
			sel?.addRange(range);
		}
	});
}

function commitEditing(a: Annotation, el: HTMLElement) {
	if (a.kind.kind !== "text") return;
	const content = el.innerText.replace(/​/g, "");
	editingId = null;
	// Emptied text → drop it rather than leave an invisible layer the canvas
	// hit-test can't select (only removable from the layer panel otherwise).
	if (content.trim() === "") {
		store.removeAnnotation(a.id);
		return;
	}
	if (a.kind.content !== content) {
		store.pushUndoState();
		store.updateAnnotation(a.id, { kind: { ...a.kind, content } });
	}
}

function handleKeyDown(e: KeyboardEvent, a: Annotation) {
	if (e.key === "Escape") {
		e.preventDefault();
		// Cancel: restore the pre-edit text before blur so commit sees no change
		// (Svelte won't reset the contenteditable when the store value is equal).
		const el = e.currentTarget as HTMLElement;
		if (a.kind.kind === "text") el.innerText = editStartContent;
		el.blur();
	}
}

function handleTextPointerDown(e: PointerEvent, a: Annotation) {
	if (editingId === a.id) return; // let contenteditable take the gesture
	if (a.locked || a.kind.kind !== "text") return;
	if (e.button !== 0) return;
	// Text dragging only on the Annotations tab so it doesn't fight the canvas/focus overlay.
	if (store.activePanel !== "annotations") return;

	e.stopPropagation();
	e.preventDefault();
	const target = e.currentTarget as HTMLElement;
	target.setPointerCapture(e.pointerId);

	const t = playbackTime();
	const pointerUV = pointerToUV(a, e, t);
	const startCss = pointerToCss(e);

	drag = {
		id: a.id,
		startX: a.kind.x,
		startY: a.kind.y,
		pointerStartUV: pointerUV,
		moved: false,
	};
	// Selecting on press matches Figma/Keynote, so the rest of the panel
	// updates immediately even before the user commits to a drag. Undo is
	// pushed on the first real move (below), so a pure select doesn't bloat it.
	store.selectedAnnotationId = a.id;

	// Stash the press position on the element for the threshold check.
	target.dataset.dragStartX = String(startCss.x);
	target.dataset.dragStartY = String(startCss.y);
}

function handleTextPointerMove(e: PointerEvent, a: Annotation) {
	if (!drag || drag.id !== a.id) return;
	if (a.kind.kind !== "text") return;

	const t = playbackTime();
	const css = pointerToCss(e);
	const target = e.currentTarget as HTMLElement;
	const startX = +(target.dataset.dragStartX ?? "0");
	const startY = +(target.dataset.dragStartY ?? "0");
	const moved = Math.hypot(css.x - startX, css.y - startY) >= CLICK_DRAG_THRESHOLD_PX;
	if (!moved && !drag.moved) return;
	// Push undo once, when the drag actually starts moving.
	if (!drag.moved) store.pushUndoState();
	drag.moved = true;

	const rawUv = pointerToUV(a, e, t);
	const dx = rawUv.x - drag.pointerStartUV.x;
	const dy = rawUv.y - drag.pointerStartUV.y;
	let nx = drag.startX + dx;
	let ny = drag.startY + dy;

	// Snap (Alt held bypasses, matching the canvas overlay).
	if (!e.altKey && store.annotationSnapEnabled) {
		const anchors = buildAnnotationSnapAnchors(store.annotations, drag.id);
		const result = snap(nx, ny, anchors, 0.005, true);
		nx = result.x;
		ny = result.y;
	}

	store.updateAnnotation(a.id, {
		kind: { ...a.kind, x: nx, y: ny },
	});
}

function handleTextPointerUp(e: PointerEvent, a: Annotation) {
	const target = e.currentTarget as HTMLElement;
	try {
		target.releasePointerCapture(e.pointerId);
	} catch {
		// capture may have already been released by the browser, so ignore.
	}
	delete target.dataset.dragStartX;
	delete target.dataset.dragStartY;
	drag = null;
	void a;
}
</script>

<div
  bind:this={layerEl}
  class="pointer-events-none absolute inset-0 overflow-hidden"
  class:hidden={store.annotationsGloballyHidden}
>
  {#each store.annotationsByZ as a (a.id)}
    {#if a.kind.kind === "text" && !a.hidden}
      {@const isEditing = editingId === a.id}
      {@const isSelected = a.id === store.selectedAnnotationId}
      {@const isActiveTab = store.activePanel === "annotations"}
      {@const interactive = isActiveTab && !a.locked}
      {@const isDragging = drag?.id === a.id && drag?.moved}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        data-text-anno-id={a.id}
        class="absolute origin-top-left select-none whitespace-pre-wrap wrap-break-word"
        class:outline={isSelected && isActiveTab}
        class:outline-1={isSelected && isActiveTab}
        class:outline-dashed={isSelected && isActiveTab && !isEditing}
        class:outline-primary={isSelected && isActiveTab}
        class:cursor-text={isEditing}
        class:cursor-grab={interactive && !isEditing && !isDragging}
        class:cursor-grabbing={isDragging}
        contenteditable={isEditing}
        style={styleFor(a)}
        onpointerdown={(e) => handleTextPointerDown(e, a)}
        onpointermove={(e) => handleTextPointerMove(e, a)}
        onpointerup={(e) => handleTextPointerUp(e, a)}
        onpointercancel={(e) => handleTextPointerUp(e, a)}
        ondblclick={(e) => {
          if (!interactive) return;
          e.stopPropagation();
          startEditing(a);
        }}
        onclick={(e) => {
          if (!interactive) return;
          if (isEditing) return;
          // Suppress the click that tails a successful drag.
          if (drag?.id === a.id && drag?.moved) {
            e.stopPropagation();
            return;
          }
          e.stopPropagation();
          store.selectedAnnotationId = a.id;
        }}
        onblur={(e) => commitEditing(a, e.currentTarget as HTMLElement)}
        onkeydown={(e) => handleKeyDown(e, a)}
        style:pointer-events={interactive ? "auto" : "none"}
        style:touch-action={interactive ? "none" : "auto"}
      >{a.kind.content}</div>
    {/if}
  {/each}
</div>
