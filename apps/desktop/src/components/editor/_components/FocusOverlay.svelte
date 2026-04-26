<script lang="ts">
  import {
    framePaddingPixels,
    type EditorStore,
    type ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import { onDestroy, onMount } from "svelte";

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    /** The container wrapping the WebGL preview — we stretch to fit its rect. */
    targetEl: HTMLElement | null;
  }

  let { store, videoEl, targetEl }: Props = $props();

  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rafHandle: number | null = null;
  let resizeObserver: ResizeObserver | null = null;

  type HandleName = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "body";
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

  const HANDLE_RADIUS_PX = 6;
  const SELECTION_COLOUR = "#3b82f6";
  const MIN_SCALE = 1.05;
  const MAX_SCALE = 3;

  function getDpr(): number {
    return window.devicePixelRatio || 1;
  }

  function compW(): number {
    const meta = store.metadata;
    if (!meta) return 0;
    const paddingPx = framePaddingPixels(store.padding, meta);
    return meta.width + paddingPx * 2;
  }

  /** Canvas device-px rect of the video region (mirror of the shader). */
  function videoRectPx(): { x: number; y: number; w: number; h: number } {
    if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
    const cw = canvasEl.width;
    const ch = canvasEl.height;
    const total = compW();
    const meta = store.metadata;
    const sourcePaddingPx = meta ? framePaddingPixels(store.padding, meta) : 0;
    const padPx = total > 0 ? (sourcePaddingPx / total) * cw : 0;
    return { x: padPx, y: padPx, w: cw - 2 * padPx, h: ch - 2 * padPx };
  }

  function uvToCanvas(ux: number, uy: number): { x: number; y: number } {
    const rect = videoRectPx();
    return { x: rect.x + ux * rect.w, y: rect.y + uy * rect.h };
  }

  function canvasToUV(cx: number, cy: number): { x: number; y: number } {
    const rect = videoRectPx();
    if (rect.w <= 0 || rect.h <= 0) return { x: 0, y: 0 };
    return { x: (cx - rect.x) / rect.w, y: (cy - rect.y) / rect.h };
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

  /** The UV-space box the focus rectangle occupies on the source frame. */
  function regionBox(r: ZoomRegion): { x: number; y: number; w: number; h: number } {
    const s = Math.max(1.001, r.scale);
    const w = 1 / s;
    const h = 1 / s;
    // Clamp so the rect stays inside [0,1]².
    const cx = Math.min(Math.max(r.centerX, w / 2), 1 - w / 2);
    const cy = Math.min(Math.max(r.centerY, h / 2), 1 - h / 2);
    return { x: cx - w / 2, y: cy - h / 2, w, h };
  }

  function handlePositions(
    x: number,
    y: number,
    w: number,
    h: number,
  ): Record<Exclude<HandleName, "body">, { x: number; y: number }> {
    return {
      nw: { x, y },
      n: { x: x + w / 2, y },
      ne: { x: x + w, y },
      e: { x: x + w, y: y + h / 2 },
      se: { x: x + w, y: y + h },
      s: { x: x + w / 2, y: y + h },
      sw: { x, y: y + h },
      w: { x, y: y + h / 2 },
    };
  }

  function hitTestHandle(
    pt: { x: number; y: number },
    x: number,
    y: number,
    w: number,
    h: number,
  ): HandleName | null {
    const dpr = getDpr();
    const slop = HANDLE_RADIUS_PX * dpr + 2 * dpr;
    const handles = handlePositions(x, y, w, h);
    for (const [name, p] of Object.entries(handles)) {
      if (Math.abs(pt.x - p.x) <= slop && Math.abs(pt.y - p.y) <= slop) {
        return name as HandleName;
      }
    }
    if (pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h) return "body";
    return null;
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

  function cursorForHandle(h: HandleName | null): string {
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
      case "body":
        return "move";
      default:
        return "";
    }
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

    const hit = hitTestHandle(pt, tl.x, tl.y, w, h);
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
      const hit = hitTestHandle(pt, tl.x, tl.y, br.x - tl.x, br.y - tl.y);
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
      // Resize: compute the new focus rect from the dragged edge, then derive
      // scale = 1 / max(rectW, rectH) and re-centre. Clamped so the rect
      // never leaves [0,1]² and the scale stays in [MIN_SCALE, MAX_SCALE].
      const uv = canvasToUV(pt.x, pt.y);
      const halfW0 = 1 / (2 * drag.startScale);
      const halfH0 = 1 / (2 * drag.startScale);
      let x0 = drag.startCX - halfW0;
      let y0 = drag.startCY - halfH0;
      let x1 = drag.startCX + halfW0;
      let y1 = drag.startCY + halfH0;

      const h = drag.handle;
      if (h === "w" || h === "nw" || h === "sw") x0 = uv.x;
      if (h === "e" || h === "ne" || h === "se") x1 = uv.x;
      if (h === "n" || h === "nw" || h === "ne") y0 = uv.y;
      if (h === "s" || h === "sw" || h === "se") y1 = uv.y;

      const rawW = Math.max(1 / MAX_SCALE, Math.abs(x1 - x0));
      const rawH = Math.max(1 / MAX_SCALE, Math.abs(y1 - y0));
      // Keep the rect square — the zoom itself is uniform scale — by taking
      // the larger of the two dimensions.
      const side = Math.min(1, Math.max(rawW, rawH, 1 / MAX_SCALE));
      const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, 1 / side));

      // Re-centre around the midpoint of the dragged rect, then clamp into
      // the frame given the new side length.
      const midX = (Math.min(x0, x1) + Math.max(x0, x1)) * 0.5;
      const midY = (Math.min(y0, y1) + Math.max(y0, y1)) * 0.5;
      const half = side * 0.5;
      const cx = Math.min(Math.max(midX, half), 1 - half);
      const cy = Math.min(Math.max(midY, half), 1 - half);

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

  // Reactive triggers — the RAF loop already picks up store changes each
  // frame, but touching them here keeps the effect graph wired for Svelte 5.
  $effect(() => {
    void store.selectedZoomRegionId;
    void store.zoomRegions;
    void store.padding;
  });

  // Editing chrome (dashed rect, handles, crosshair) is only meaningful while
  // the user is on the Focus tab — otherwise the overlay both hides itself and
  // stops swallowing pointer events so clicks reach the AnnotationOverlay or
  // the preview underneath.
  const isActive = $derived(
    store.activePanel === "focus" && store.selectedZoomRegionId !== null,
  );
</script>

{#if store.activePanel === "focus"}
  <canvas
    bind:this={canvasEl}
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
    class="pointer-events-auto absolute inset-0 h-full w-full"
    class:pointer-events-none={!isActive}
    style="touch-action: none;"
  ></canvas>
{/if}
