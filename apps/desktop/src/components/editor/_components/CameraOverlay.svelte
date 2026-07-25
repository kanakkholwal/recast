<script lang="ts">
  import { computeCanvasGeometry } from "$lib/canvas-geometry";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { evaluateZoomAt } from "../video-preview.logic";
  import {
    applyZoomFollow,
    bubblePlacementStyle,
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
  }

  let { store, videoEl, targetEl, cameraSrc }: Props = $props();

  let cameraVideoEl: HTMLVideoElement | null = $state(null);

  const geom = $derived.by(() => {
    const m = store.metadata;
    if (!m || !m.width || !m.height) return null;
    return computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
  });

  // The rendered placement = the saved base, then the zoom-follow effect (grow +
  // drift away from the active zoom's focus). Editing writes the BASE; this only
  // shifts what's drawn. Identity when zoom-follow is off, focus is bypassed, or
  // no zoom is active at the playhead.
  const effectivePlacement = $derived.by(() => {
    const base = store.cameraOverlay.defaultPlacement;
    if (!store.cameraOverlay.zoomFollow || !store.focusEnabled) return base;
    const zoom = evaluateZoomAt(store.zoomRegions, store.currentTime);
    return applyZoomFollow(base, zoom, {
      enabled: true,
      strength: store.cameraOverlay.zoomFollowStrength,
    });
  });

  const bubbleStyle = $derived(bubblePlacementStyle(geom, effectivePlacement));
  const borderRadius = $derived(
    shapeBorderRadius(store.cameraOverlay.shape, store.cameraOverlay.cornerRadius),
  );

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
    const p = store.cameraOverlay.defaultPlacement;
    dragStartUv = { x: p.x, y: p.y };
    store.pushUndoState();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function onPointerMove(e: PointerEvent) {
    if (!isDragging || !targetEl || !geom) return;
    const rect = targetEl.getBoundingClientRect();
    const p = store.cameraOverlay.defaultPlacement;
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
    store.updateCameraOverlay({ defaultPlacement: { ...p, x: next.x, y: next.y } });
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
    store.updateCameraOverlay({
      defaultPlacement: resizeCameraSquare(store.cameraOverlay.defaultPlacement, corner, uv.x, uv.y),
    });
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
    style="{bubbleStyle} aspect-ratio: 1; cursor: {isDragging ? 'grabbing' : 'grab'}; touch-action: none;"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    <div
      class="h-full w-full overflow-hidden"
      style="border-radius: {borderRadius}; box-shadow: 0 6px 22px rgba(0, 0, 0, 0.32);"
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
