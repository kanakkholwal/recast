<script lang="ts">
  import { computeCanvasGeometry } from "$lib/canvas-geometry";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    bubblePlacementStyle,
    clampCameraDrag,
    shapeBorderRadius,
  } from "./camera-overlay.logic";

  interface Props {
    store: EditorStore;
    /** The screen video element, used as the time-base for camera sync. */
    videoEl: HTMLVideoElement | null;
    /** Preview rectangle (canvas-sized div) — positioning parent and drag-coord reference. */
    targetEl: HTMLDivElement | null;
    /** `convertFileSrc(camera.mp4)`, or empty when no camera was recorded (renders nothing). */
    cameraSrc: string;
  }

  let { store, videoEl, targetEl, cameraSrc }: Props = $props();

  let cameraVideoEl: HTMLVideoElement | null = $state(null);

  // Bubble UV is in *video* space (so "bottom-right of the video", not of the padded
  // canvas); transformed into canvas-pixel offsets here.
  const geom = $derived.by(() => {
    const m = store.metadata;
    if (!m || !m.width || !m.height) return null;
    return computeCanvasGeometry(
      m.width,
      m.height,
      store.padding,
      store.outputAspect,
    );
  });

  const bubbleStyle = $derived(
    bubblePlacementStyle(geom, store.cameraOverlay.defaultPlacement),
  );

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

  // Play/pause in lockstep; set currentTime to the screen's instant before play so
  // the first frame is correct even after a long pause.
  $effect(() => {
    const playing = store.isPlaying;
    if (!cameraVideoEl) return;
    if (playing) {
      if (videoEl) cameraVideoEl.currentTime = videoEl.currentTime;
      void cameraVideoEl.play().catch((err) => {
        // Network/decoder hiccups can still throw; keep the screen video playing.
        console.warn("camera overlay play failed:", err);
      });
    } else {
      cameraVideoEl.pause();
    }
  });

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
    store.updateCameraOverlay({
      defaultPlacement: { ...p, x: next.x, y: next.y },
    });
  }

  function onPointerUp(e: PointerEvent) {
    if (!isDragging) return;
    isDragging = false;
    try {
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Ignore — pointer capture may already have been released.
    }
  }
</script>

{#if cameraSrc && store.cameraOverlay.enabled && geom}
  <!-- Bubble wrapper owns position, shape, shadow, and drag pointers; the <video> fills it via object-fit:cover. -->
  <div
    role="presentation"
    class="absolute select-none"
    style="
      {bubbleStyle}
      aspect-ratio: 1;
      border-radius: {borderRadius};
      overflow: hidden;
      box-shadow: 0 6px 22px rgba(0, 0, 0, 0.32);
      cursor: {isDragging ? 'grabbing' : 'grab'};
      touch-action: none;
    "
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={cameraVideoEl}
      src={cameraSrc}
      muted
      playsinline
      preload="auto"
      class="block h-full w-full"
      style="
        object-fit: cover;
        transform: {store.cameraOverlay.mirror ? 'scaleX(-1)' : 'none'};
        pointer-events: none;
      "
    ></video>
  </div>
{/if}
