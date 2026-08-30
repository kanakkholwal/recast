<script lang="ts">
import { onDestroy, onMount } from "svelte";
import { type EditorStore, type ZoomRegion } from "../../stores/editor-store.svelte";
import {
	canvasToUV as canvasToUVPure,
	cursorForHandle,
	HANDLE_RADIUS_PX,
	type HandleName,
	handlePositions,
	hitTestHandle,
	MAX_SCALE,
	MIN_SCALE,
	regionBox,
	resizeFocusRegion,
	uvToCanvas as uvToCanvasPure,
	videoRectPx as videoRectPxPure,
} from "./focus-overlay.logic";

interface Props {
	store: EditorStore;
	videoEl: HTMLVideoElement | null;
	/** The container wrapping the WebGL preview, which we stretch to fit its rect. */
	targetEl: HTMLElement | null;
}

let { store, videoEl, targetEl }: Props = $props();

let canvasEl: HTMLCanvasElement | null = $state(null);
let rafHandle: number | null = null;
let resizeObserver: ResizeObserver | null = null;

type DragState =
	| null
	| {
			kind: "move";
			id: string;
			startCX: number;
			startCY: number;
			pointerStartUV: { x: number; y: number };
	  }
	| {
			kind: "resize";
			id: string;
			handle: HandleName;
			startScale: number;
			startCX: number;
			startCY: number;
	  };
let drag: DragState = null;

const SELECTION_COLOUR = "#3b82f6";

function getDpr(): number {
	return window.devicePixelRatio || 1;
}

// Thin wrappers binding canvas dims and the store to the shared projections, so call sites stay two-arg.
function videoRectPx(): { x: number; y: number; w: number; h: number } {
	if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
	return videoRectPxPure(canvasEl.width, canvasEl.height, store.metadata, store.padding);
}

function uvToCanvas(ux: number, uy: number): { x: number; y: number } {
	return uvToCanvasPure(ux, uy, videoRectPx());
}

function canvasToUV(cx: number, cy: number): { x: number; y: number } {
	return canvasToUVPure(cx, cy, videoRectPx());
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

function selectedRegion(): ZoomRegion | null {
	const id = store.selectedZoomRegionId;
	if (!id) return null;
	return store.zoomRegions.find((r) => r.id === id) ?? null;
}

function resizeToContainer() {
	if (!canvasEl || !targetEl) return;
	const rect = targetEl.getBoundingClientRect();
	const dpr = getDpr();
	const w = Math.max(1, Math.floor(rect.width * dpr));
	const h = Math.max(1, Math.floor(rect.height * dpr));
	if (canvasEl.width !== w || canvasEl.height !== h) {
		canvasEl.width = w;
		canvasEl.height = h;
	}
}

function draw() {
	if (!canvasEl) return;
	resizeToContainer();
	const ctx = canvasEl.getContext("2d");
	if (!ctx) return;
	ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

	const r = selectedRegion();
	if (!r) return;

	const box = regionBox(r);
	const tl = uvToCanvas(box.x, box.y);
	const br = uvToCanvas(box.x + box.w, box.y + box.h);
	const x = tl.x;
	const y = tl.y;
	const w = br.x - tl.x;
	const h = br.y - tl.y;
	if (w <= 0 || h <= 0) return;

	const dpr = getDpr();

	ctx.save();
	ctx.strokeStyle = SELECTION_COLOUR;
	ctx.lineWidth = 1.5 * dpr;
	ctx.setLineDash([4 * dpr, 3 * dpr]);
	ctx.strokeRect(x, y, w, h);
	ctx.setLineDash([]);

	// Crosshair at focus centre.
	const cx = (tl.x + br.x) * 0.5;
	const cy = (tl.y + br.y) * 0.5;
	const arm = 6 * dpr;
	ctx.beginPath();
	ctx.moveTo(cx - arm, cy);
	ctx.lineTo(cx + arm, cy);
	ctx.moveTo(cx, cy - arm);
	ctx.lineTo(cx, cy + arm);
	ctx.lineWidth = 1.5 * dpr;
	ctx.stroke();

	// 8 resize handles.
	const hs = HANDLE_RADIUS_PX * dpr;
	const handles = handlePositions(x, y, w, h);
	for (const pt of Object.values(handles)) {
		ctx.fillStyle = "#ffffff";
		ctx.fillRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
		ctx.strokeStyle = SELECTION_COLOUR;
		ctx.lineWidth = 1.5 * dpr;
		ctx.strokeRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
	}
	ctx.restore();
}

function tick() {
	draw();
	rafHandle = requestAnimationFrame(tick);
}

function handlePointerDown(e: PointerEvent) {
	const r = selectedRegion();
	if (!r || !canvasEl) return;
	const pt = pointerToCanvasPx(e);
	const box = regionBox(r);
	const tl = uvToCanvas(box.x, box.y);
	const br = uvToCanvas(box.x + box.w, box.y + box.h);
	const w = br.x - tl.x;
	const h = br.y - tl.y;

	const hit = hitTestHandle(pt, tl.x, tl.y, w, h, getDpr());
	if (!hit) return;

	(e.currentTarget as Element).setPointerCapture(e.pointerId);
	store.pushUndoState();

	if (hit === "body") {
		const pointerUV = canvasToUV(pt.x, pt.y);
		drag = {
			kind: "move",
			id: r.id,
			startCX: r.centerX,
			startCY: r.centerY,
			pointerStartUV: pointerUV,
		};
	} else {
		drag = {
			kind: "resize",
			id: r.id,
			handle: hit,
			startScale: r.scale,
			startCX: r.centerX,
			startCY: r.centerY,
		};
	}
	e.preventDefault();
}

function handlePointerMove(e: PointerEvent) {
	if (!canvasEl) return;

	if (!drag) {
		// Hover cursor feedback only when a region is selected.
		const r = selectedRegion();
		if (!r) {
			canvasEl.style.cursor = "";
			return;
		}
		const pt = pointerToCanvasPx(e);
		const box = regionBox(r);
		const tl = uvToCanvas(box.x, box.y);
		const br = uvToCanvas(box.x + box.w, box.y + box.h);
		const hit = hitTestHandle(pt, tl.x, tl.y, br.x - tl.x, br.y - tl.y, getDpr());
		canvasEl.style.cursor = cursorForHandle(hit);
		return;
	}

	const r = store.zoomRegions.find((z) => z.id === drag!.id);
	if (!r) return;
	const pt = pointerToCanvasPx(e);

	if (drag.kind === "move") {
		const uv = canvasToUV(pt.x, pt.y);
		const dx = uv.x - drag.pointerStartUV.x;
		const dy = uv.y - drag.pointerStartUV.y;
		const half = 1 / (2 * Math.max(1.001, r.scale));
		const cx = Math.min(Math.max(drag.startCX + dx, half), 1 - half);
		const cy = Math.min(Math.max(drag.startCY + dy, half), 1 - half);
		store.updateZoomRegion(r.id, { centerX: cx, centerY: cy });
		return;
	}

	if (drag.kind === "resize") {
		const uv = canvasToUV(pt.x, pt.y);
		const { scale, cx, cy } = resizeFocusRegion(
			drag.handle,
			drag.startScale,
			drag.startCX,
			drag.startCY,
			uv,
			{ min: MIN_SCALE, max: MAX_SCALE },
		);
		store.updateZoomRegion(r.id, { scale, centerX: cx, centerY: cy });
	}
}

function handlePointerUp(e: PointerEvent) {
	if (drag) {
		try {
			(e.currentTarget as Element).releasePointerCapture(e.pointerId);
		} catch {}
		drag = null;
	}
}

onMount(() => {
	tick();
	if (targetEl) {
		resizeObserver = new ResizeObserver(() => {
			if (canvasEl) resizeToContainer();
		});
		resizeObserver.observe(targetEl);
	}
});

onDestroy(() => {
	if (rafHandle !== null) cancelAnimationFrame(rafHandle);
	resizeObserver?.disconnect();
});

// The rAF loop already reads the store each frame; touching these keeps the effect graph wired.
$effect(() => {
	void store.selectedZoomRegionId;
	void store.zoomRegions;
	void store.padding;
});

// Off the Focus tab the overlay hides and stops swallowing pointer events, so clicks reach the layers beneath.
const isActive = $derived(store.activePanel === "focus" && store.selectedZoomRegionId !== null);
</script>

{#if store.activePanel === "focus"}
  <!-- aria-hidden for the same reason as FocusPad: the drag handles are a
       pointer convenience, and FocusPanel's scale / centre-X / centre-Y
       sliders are the keyboard-complete way to set the same three values. -->
  <canvas
    bind:this={canvasEl}
    aria-hidden="true"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
    class="pointer-events-auto absolute inset-0 h-full w-full"
    class:pointer-events-none={!isActive}
    style="touch-action: none;"
  ></canvas>
{/if}
