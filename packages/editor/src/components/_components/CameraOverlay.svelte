<script lang="ts">
import { computeCanvasGeometry } from "../../lib/canvas-geometry";
import type { EditorStore } from "../../stores/editor-store.svelte";
import {
	applyZoomFollow,
	bubblePlacementStyle,
	type CameraResizeCorner,
	cameraBubbleDelta,
	cameraFollowScaleAt,
	cameraPlacementAt,
	clampCameraDrag,
	resizeCameraSquare,
	shapeBorderRadius,
} from "./camera-overlay.logic";

interface Props {
	store: EditorStore;
	/** The screen video element, used as the time-base for camera sync. */
	/** Preview rectangle (canvas-sized div): positioning parent and drag-coord reference. */
	targetEl: HTMLDivElement | null;
	/** `convertFileSrc(camera.mp4)`, or empty when no camera was recorded (renders nothing). */
	/** Whether this project has a camera track at all. The feed itself belongs to
	 *  the preview, which hands it to the compositor. */
	hasCamera: boolean;
	/** Per-frame picture time (unthrottled) so the zoom-follow grow is as smooth
	 *  as the shader; falls back to store.currentTime when paused. */
	previewTime?: number;
	/** Milliseconds the camera track starts after video frame 0, measured at
	 *  capture. 0 for projects recorded before it was measured. */
}

let { store, targetEl, hasCamera, previewTime = 0 }: Props = $props();

const geom = $derived.by(() => {
	const m = store.metadata;
	if (!m?.width || !m.height) return null;
	return computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
});

// The bubble is square in pixels, so its UV height is width times aspect; resize and zoom-follow both use it.
const videoAspect = $derived(geom ? geom.videoW / geom.videoH : 1);

// Rendered placement is the saved base plus zoom-follow; editing writes the BASE, and this only shifts what is drawn.
function baseAt(t: number) {
	return cameraPlacementAt(
		store.cameraOverlay.defaultPlacement,
		store.cameraOverlay.keyframes,
		t,
		store.cameraOverlay.keyframeEasing,
	);
}

// The LAYOUT box deliberately ignores the playhead: the 25 Hz store clock and the per-rAF transform landing in different frames made the bubble judder.
const layoutPlacement = $derived(store.cameraOverlay.defaultPlacement);
const bubbleStyle = $derived(bubblePlacementStyle(geom, layoutPlacement));
const borderRadius = $derived(
	shapeBorderRadius(store.cameraOverlay.shape, store.cameraOverlay.cornerRadius),
);

let outerEl: HTMLDivElement | null = $state(null);

// Written imperatively as a transform so growth is GPU-composited once per rAF in lockstep with the display, never triggering layout.
function followTransform(t: number): string {
	// Everything the playhead affects lives in this transform, so the layout box never moves underneath it.
	const layout = layoutPlacement;
	if (layout.width <= 0) return "translateZ(0)";
	const b = baseAt(t);
	let e = b;
	if (store.cameraOverlay.zoomFollow && store.focusEnabled) {
		const zoom = cameraFollowScaleAt(
			store.zoomRegions,
			t,
			store.cameraOverlay.zoomFollowDuration,
			store.cameraOverlay.zoomFollowEasing,
		);
		e = applyZoomFollow(
			b,
			zoom,
			{ enabled: true, strength: store.cameraOverlay.zoomFollowStrength },
			videoAspect,
		);
	}
	const d = cameraBubbleDelta(layout, e, videoAspect);
	return `translate(${d.tx.toFixed(4)}%, ${d.ty.toFixed(4)}%) scale(${d.scale.toFixed(5)}) translateZ(0)`;
}

// Paused, scrub or edit: a reactive write so the bubble tracks the playhead and edits exactly.
$effect(() => {
	if (store.isPlaying || !outerEl) return;
	outerEl.style.transform = followTransform(store.currentTime);
});

// Playing: an rAF loop off the unthrottled picture clock, so the grow matches the shader with no reactive-flush hop.
$effect(() => {
	if (!store.isPlaying) return;
	const el = outerEl;
	if (!el) return;
	let raf = requestAnimationFrame(function tick() {
		el.style.transform = followTransform(previewTime);
		raf = requestAnimationFrame(tick);
	});
	return () => cancelAnimationFrame(raf);
});

// Client px → video UV (for absolute resize math), via the canvas rect + geom.
function clientToVideoUv(clientX: number, clientY: number): { x: number; y: number } | null {
	if (!targetEl || !geom) return null;
	const rect = targetEl.getBoundingClientRect();
	if (rect.width <= 0 || rect.height <= 0) return null;
	const canvasX = ((clientX - rect.left) / rect.width) * geom.canvasW;
	const canvasY = ((clientY - rect.top) / rect.height) * geom.canvasH;
	return {
		x: (canvasX - geom.videoX) / geom.videoW,
		y: (canvasY - geom.videoY) / geom.videoH,
	};
}

// UV deltas are relative to the rendered video rect so padding doesn't bias motion; one undo entry per drag.
let isDragging = $state(false);
let dragStartClient = { x: 0, y: 0 };
let dragStartUv = { x: 0, y: 0 };

function onPointerDown(e: PointerEvent) {
	if (!targetEl || !geom) return;
	isDragging = true;
	dragStartClient = { x: e.clientX, y: e.clientY };
	const p = baseAt(store.currentTime);
	dragStartUv = { x: p.x, y: p.y };
	store.pushUndoState();
	(e.target as HTMLElement).setPointerCapture(e.pointerId);
	e.preventDefault();
}

function onPointerMove(e: PointerEvent) {
	if (!isDragging || !targetEl || !geom) return;
	const rect = targetEl.getBoundingClientRect();
	const p = baseAt(store.currentTime);
	const next = clampCameraDrag(
		geom,
		rect.width,
		rect.height,
		e.clientX - dragStartClient.x,
		e.clientY - dragStartClient.y,
		dragStartUv,
		p,
	);
	if (!next) return;
	// Routes to a keyframe at the playhead in per-cut mode, else defaultPlacement.
	store.setCameraPlacement({ ...p, x: next.x, y: next.y });
}

function onPointerUp(e: PointerEvent) {
	if (!isDragging) return;
	isDragging = false;
	try {
		(e.target as HTMLElement).releasePointerCapture(e.pointerId);
	} catch {
		// Ignore, pointer capture may already have been released.
	}
}

// Corner resize. Anchors the opposite corner; square-locked (see resizeCameraSquare).
let resizing = $state<CameraResizeCorner | null>(null);
// Handles are size-3 (12px); a -6px offset centres each on its corner.
const CORNERS: Array<{ id: CameraResizeCorner; offset: string; cursor: string }> = [
	{ id: "tl", offset: "left:-6px;top:-6px", cursor: "nwse-resize" },
	{ id: "tr", offset: "right:-6px;top:-6px", cursor: "nesw-resize" },
	{ id: "bl", offset: "left:-6px;bottom:-6px", cursor: "nesw-resize" },
	{ id: "br", offset: "right:-6px;bottom:-6px", cursor: "nwse-resize" },
];

function onHandleDown(e: PointerEvent, corner: CameraResizeCorner) {
	if (!targetEl || !geom) return;
	e.stopPropagation();
	e.preventDefault();
	resizing = corner;
	store.pushUndoState();
	(e.target as HTMLElement).setPointerCapture(e.pointerId);
}

function onHandleMove(e: PointerEvent, corner: CameraResizeCorner) {
	if (resizing !== corner) return;
	const uv = clientToVideoUv(e.clientX, e.clientY);
	if (!uv) return;
	store.setCameraPlacement(
		resizeCameraSquare(baseAt(store.currentTime), corner, uv.x, uv.y, videoAspect),
	);
}

function onHandleUp(e: PointerEvent) {
	if (resizing === null) return;
	resizing = null;
	try {
		(e.target as HTMLElement).releasePointerCapture(e.pointerId);
	} catch {
		// Ignore.
	}
}
</script>

{#if hasCamera && store.cameraOverlay.enabled && geom}
  <!-- Outer bounds: owns position + drag + resize handles (NOT clipped, so handles
       show past the shape). The inner div clips the video to the bubble shape. -->
  <div
    role="presentation"
    class="group absolute select-none"
    bind:this={outerEl}
    style="{bubbleStyle} aspect-ratio: 1; container-type: size; transform-origin: 0 0; will-change: transform; cursor: {isDragging ? 'grabbing' : 'grab'}; touch-action: none;"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    <!-- The compositor draws the bubble. This is its hit target: same rect, same
         shape, no pixels of its own. -->
    <div class="h-full w-full" style="border-radius: {borderRadius};"></div>

    <!-- Not buttons: they had no onclick and four identical accessible names, so
         they were four dead tab stops. Drag-only by nature; CameraPanel's "Bubble
         size" slider and preset grid are the keyboard-complete equivalents. -->
    {#each CORNERS as c (c.id)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        aria-hidden="true"
        class="absolute size-3 rounded-full border border-white/80 bg-primary opacity-0 shadow transition-opacity group-hover:opacity-100"
        style="{c.offset}; cursor: {c.cursor}; touch-action: none;"
        onpointerdown={(e) => onHandleDown(e, c.id)}
        onpointermove={(e) => onHandleMove(e, c.id)}
        onpointerup={onHandleUp}
        onpointercancel={onHandleUp}
      ></span>
    {/each}
  </div>
{/if}
