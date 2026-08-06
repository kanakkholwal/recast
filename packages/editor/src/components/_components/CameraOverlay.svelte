<script lang="ts">
import { computeCanvasGeometry } from "../../lib/canvas-geometry";
import type { EditorStore } from "../../stores/editor-store.svelte";
import {
	applyZoomFollow,
	bubblePlacementStyle,
	cameraFollowScaleAt,
	cameraPlacementAt,
	cameraShadowStyle,
	type CameraResizeCorner,
	clampCameraDrag,
	resizeCameraSquare,
	shapeBorderRadius,
} from "./camera-overlay.logic";

interface Props {
	store: EditorStore;
	/** The screen video element, used as the time-base for camera sync. */
	videoEl: HTMLVideoElement | null;
	/** Preview rectangle (canvas-sized div): positioning parent and drag-coord reference. */
	targetEl: HTMLDivElement | null;
	/** `convertFileSrc(camera.mp4)`, or empty when no camera was recorded (renders nothing). */
	cameraSrc: string;
	/** Per-frame picture time (unthrottled) so the zoom-follow grow is as smooth
	 *  as the shader; falls back to store.currentTime when paused. */
	previewTime?: number;
}

let { store, videoEl, targetEl, cameraSrc, previewTime = 0 }: Props = $props();

let cameraVideoEl: HTMLVideoElement | null = $state(null);

const geom = $derived.by(() => {
	const m = store.metadata;
	if (!m || !m.width || !m.height) return null;
	return computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
});

// Video pixel aspect. The bubble is square in pixels, so its UV height is
// `width * aspect` — used by resize + zoom-follow so nothing distorts or
// mis-anchors on a non-square (e.g. 16:9) recording.
const videoAspect = $derived(geom ? geom.videoW / geom.videoH : 1);

// The rendered placement = the saved base, then the zoom-follow effect (grow +
// drift away from the active zoom's focus). Editing writes the BASE; this only
// shifts what's drawn. Identity when zoom-follow is off, focus is bypassed, or
// no zoom is active at the playhead.
// Base placement at original time `t`: the static defaultPlacement, or the
// per-cut keyframes gliding between positions.
function baseAt(t: number) {
	return cameraPlacementAt(
		store.cameraOverlay.defaultPlacement,
		store.cameraOverlay.keyframes,
		t,
		store.cameraOverlay.keyframeEasing,
	);
}

// Drop shadow in cqmin (the outer div is a size container), so it scales with
// the bubble and mirrors the export's render_camera_shadow.
const shadowStyle = $derived(cameraShadowStyle(store.cameraOverlay.shadow ?? 0));

// BASE placement (no zoom-follow): drives the div's LAYOUT box. Uses the store
// clock (paused-accurate; ~25 Hz while playing, fine because the base only
// MOVES on a per-cut glide). The grow/drift is applied on top via `transform`.
const basePlacement = $derived(baseAt(store.currentTime));
const bubbleStyle = $derived(bubblePlacementStyle(geom, basePlacement));
const borderRadius = $derived(
	shapeBorderRadius(store.cameraOverlay.shape, store.cameraOverlay.cornerRadius),
);

let outerEl: HTMLDivElement | null = $state(null);

// The grow/drift, expressed as a `transform` (translate% + scale) relative to
// the base box, so per-frame growth is GPU-composited and never triggers
// layout. Written IMPERATIVELY (below) rather than through Svelte reactivity so
// it updates once per rAF in lockstep with the display — same cadence as the
// zoom shader. `translateZ(0)` keeps the bubble on its own compositor layer.
function followTransform(t: number): string {
	// Reference = the LAYOUT box (bubbleStyle uses store.currentTime), so the
	// transform is a pure delta on it. Only the grow is evaluated at the smooth
	// clock `t` — the base is constant during a grow, so this stays crisp (the
	// <video> rasterises at its base size) and exact.
	const b = baseAt(store.currentTime);
	if (b.width <= 0) return "translateZ(0)";
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
	const s = e.width / b.width;
	const baseH = Math.min(1, b.width * videoAspect);
	const tx = ((e.x - b.x) / b.width) * 100;
	const ty = baseH > 0 ? ((e.y - b.y) / baseH) * 100 : 0;
	return `translate(${tx.toFixed(4)}%, ${ty.toFixed(4)}%) scale(${s.toFixed(5)}) translateZ(0)`;
}

// Paused / scrub / edit: reactive write so the bubble tracks the playhead and
// edits exactly. (Reads store.currentTime + the overlay/zoom deps.)
$effect(() => {
	if (store.isPlaying || !outerEl) return;
	outerEl.style.transform = followTransform(store.currentTime);
});

// Playing: own rAF loop writing the transform from the unthrottled picture
// clock, so the grow is as buttery as the shader (no reactive-flush hop).
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

// Keep the camera <video> within ~150ms of the screen video; the tolerance avoids
// re-seeking on micro-jitter between the two HTMLVideoElement clocks.
$effect(() => {
	void store.currentTime;
	if (!cameraVideoEl || !videoEl) return;
	if (Number.isNaN(videoEl.currentTime)) return;
	if (Math.abs(cameraVideoEl.currentTime - videoEl.currentTime) > 0.15) {
		cameraVideoEl.currentTime = videoEl.currentTime;
	}
});

$effect(() => {
	const playing = store.isPlaying;
	if (!cameraVideoEl) return;
	if (playing) {
		if (videoEl) cameraVideoEl.currentTime = videoEl.currentTime;
		void cameraVideoEl.play().catch((err) => {
			console.warn("camera overlay play failed:", err);
		});
	} else {
		cameraVideoEl.pause();
	}
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

// Drag-to-reposition. UV deltas are relative to the rendered video rect (not the
// canvas, so padding doesn't bias motion); pushUndoState at pointerdown = one undo entry.
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

{#if cameraSrc && store.cameraOverlay.enabled && geom}
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
    <div
      class="h-full w-full overflow-hidden"
      style="border-radius: {borderRadius}; box-shadow: {shadowStyle};"
    >
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={cameraVideoEl}
        src={cameraSrc}
        muted
        playsinline
        preload="auto"
        class="block h-full w-full"
        style="object-fit: cover; transform: {store.cameraOverlay.mirror
          ? 'scaleX(-1)'
          : 'none'}; pointer-events: none;"
      ></video>
    </div>

    {#each CORNERS as c (c.id)}
      <button
        type="button"
        aria-label="Resize camera"
        class="absolute size-3 rounded-full border border-white/80 bg-primary opacity-0 shadow transition-opacity group-hover:opacity-100"
        style="{c.offset}; cursor: {c.cursor}; touch-action: none;"
        onpointerdown={(e) => onHandleDown(e, c.id)}
        onpointermove={(e) => onHandleMove(e, c.id)}
        onpointerup={onHandleUp}
        onpointercancel={onHandleUp}
      ></button>
    {/each}
  </div>
{/if}
