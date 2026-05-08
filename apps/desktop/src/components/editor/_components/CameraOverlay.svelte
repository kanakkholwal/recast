<script lang="ts">
  import { computeCanvasGeometry } from "$lib/canvas-geometry";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";

  interface Props {
    store: EditorStore;
    /** The screen video element, used as the time-base for camera sync. */
    videoEl: HTMLVideoElement | null;
    /** The preview rectangle (canvas-sized div) — used as the positioning
     *  parent and the drag-coord reference. */
    targetEl: HTMLDivElement | null;
    /** `convertFileSrc(camera.mp4)` for this project, or empty when no
     *  camera was recorded. The component renders nothing when empty. */
    cameraSrc: string;
  }

  let { store, videoEl, targetEl, cameraSrc }: Props = $props();

  let cameraVideoEl: HTMLVideoElement | null = $state(null);

  // Final-canvas geometry. Drives the bubble's absolute placement inside
  // `targetEl`. The bubble's UV is in *video* space (so the user picks
  // "bottom-right of the video" not "of the canvas-with-padding"), and we
  // transform that into canvas-pixel offsets here.
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

  /**
   * Bubble CSS position. The wrapper sits inside `targetEl` (which fills
   * the canvas), so we express x/y/width as percentages of the canvas.
   *
   * Height is omitted on purpose — we use `aspect-ratio: 1` to keep the
   * bubble square on screen regardless of video aspect (a 1:1 placement
   * in UV coords would render rectangular on a 16:9 video, which is not
   * what users picking "rounded square" expect).
   */
  const bubbleStyle = $derived.by(() => {
    if (!geom) return "display:none;";
    const p = store.cameraOverlay.defaultPlacement;
    const left = ((geom.videoX + p.x * geom.videoW) / geom.canvasW) * 100;
    const top = ((geom.videoY + p.y * geom.videoH) / geom.canvasH) * 100;
    const width = ((p.width * geom.videoW) / geom.canvasW) * 100;
    return `left:${left}%;top:${top}%;width:${width}%;`;
  });

  /** border-radius derived from the saved shape. Square → 0; rounded uses
   *  the saved corner-radius (16% default); circle is 50% (with the
   *  enforced 1:1 aspect this gives a true circle). */
  const borderRadius = $derived.by(() => {
    const s = store.cameraOverlay.shape;
    if (s === "circle") return "50%";
    if (s === "square" || s === "rectangle") return "0";
    return `${(store.cameraOverlay.cornerRadius ?? 0.16) * 100}%`;
  });

  // Drift correction: keep the camera <video> within ~150 ms of the screen
  // video at all times. Re-runs whenever `store.currentTime` ticks (set by
  // the screen video's `timeupdate`) and on user scrubs (handled via the
  // route's onSeeked → store.currentTime path). The 150 ms tolerance keeps
  // micro-jitter between two HTMLVideoElement clocks from triggering
  // unnecessary seeks during normal playback.
  $effect(() => {
    void store.currentTime;
    if (!cameraVideoEl || !videoEl) return;
    if (Number.isNaN(videoEl.currentTime)) return;
    if (Math.abs(cameraVideoEl.currentTime - videoEl.currentTime) > 0.15) {
      cameraVideoEl.currentTime = videoEl.currentTime;
    }
  });

  // Play/pause the camera in lockstep with the screen video. Mirrors the
  // existing audio-element sync in the editor route — set currentTime to
  // the screen's instant before starting playback so the first frame is
  // the right one even after a long pause.
  $effect(() => {
    const playing = store.isPlaying;
    if (!cameraVideoEl) return;
    if (playing) {
      if (videoEl) cameraVideoEl.currentTime = videoEl.currentTime;
      void cameraVideoEl.play().catch((err) => {
        // Autoplay restrictions don't apply because we're muted, but
        // network/decoder hiccups can still throw — keep the screen video
        // playing in that case.
        console.warn("camera overlay play failed:", err);
      });
    } else {
      cameraVideoEl.pause();
    }
  });

  //  Drag-to-reposition
  //
  // Pointer-captured drag in the preview. `dragStartUv` snapshots the
  // placement at pointerdown and we accumulate UV-space deltas relative to
  // the rendered video rect (NOT the canvas, so padding doesn't bias the
  // motion). The single `pushUndoState` lives at pointerdown so the entire
  // drag collapses to one undo entry.
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
    if (rect.width <= 0 || rect.height <= 0) return;
    // Convert CSS-pixel drag deltas into video-UV deltas. The canvas spans
    // the full rect; the video is at (videoX, videoY) with size
    // (videoW, videoH) inside it. Drag distance / video CSS size =
    // UV delta.
    const videoCssW = rect.width * (geom.videoW / geom.canvasW);
    const videoCssH = rect.height * (geom.videoH / geom.canvasH);
    if (videoCssW <= 0 || videoCssH <= 0) return;
    const dxUv = (e.clientX - dragStartClient.x) / videoCssW;
    const dyUv = (e.clientY - dragStartClient.y) / videoCssH;
    const p = store.cameraOverlay.defaultPlacement;
    const newX = Math.max(0, Math.min(1 - p.width, dragStartUv.x + dxUv));
    const newY = Math.max(0, Math.min(1 - p.height, dragStartUv.y + dyUv));
    store.updateCameraOverlay({
      defaultPlacement: { ...p, x: newX, y: newY },
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
  <!-- Bubble wrapper — owns position, shape, shadow, and drag pointers. The
       <video> inside fills the wrapper with object-fit:cover so the camera
       feed fills the bubble cleanly regardless of camera resolution. -->
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
