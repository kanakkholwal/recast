<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { fade } from "svelte/transition";
  import {
    formatTimeByMode,
    formatTimecode,
    frameStep,
    minClipDuration,
    quantizeToFrame,
    type TimeMode,
  } from "./timeline-helpers";

  // Clip bar with thumbnails and the in/out trim handles. Owns its own
  // drag state — the parent only supplies `clientXToTime` so the
  // pointermove/up handlers can resolve absolute pointer X (already
  // including timeline scroll offset) into a clip time.

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    fps: number;
    duration: number;
    clipLeft: number;
    clipWidth: number;
    hasTrim: boolean;
    thumbnailWidth: number;
    timeMode: TimeMode;
    clientXToTime: (clientX: number) => number;
  }

  let {
    store,
    videoEl,
    fps,
    duration,
    clipLeft,
    clipWidth,
    hasTrim,
    thumbnailWidth,
    timeMode,
    clientXToTime,
  }: Props = $props();

  let activeTrimHandle = $state<"in" | "out" | null>(null);

  // Live drag context for the in/out trim handles. `originalAt` is the value
  // the handle had at pointer-down — used to display a delta in the tooltip
  // so users see exactly how many frames they've shaved off.
  let trimDragContext = $state<{
    which: "in" | "out";
    originalAt: number;
  } | null>(null);

  function startTrimDrag(event: PointerEvent, which: "in" | "out") {
    if (duration <= 0) return;
    event.preventDefault();
    event.stopPropagation();
    // Single undo entry per drag, regardless of how many pointermove events
    // fire while the user holds the handle.
    store.pushUndoState();
    activeTrimHandle = which;
    trimDragContext = {
      which,
      originalAt: which === "in" ? store.inPoint : store.outPoint,
    };
    document.body.style.cursor = "ew-resize";
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    updateTrimFromPointer(event.clientX, which, true);
    const onMove = (e: PointerEvent) => {
      updateTrimFromPointer(e.clientX, which, true);
    };
    const onUp = (e: PointerEvent) => {
      activeTrimHandle = null;
      trimDragContext = null;
      document.body.style.cursor = "";
      try {
        (event.currentTarget as Element).releasePointerCapture(e.pointerId);
      } catch {
        // already released on some browsers
      }
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function updateTrimFromPointer(
    clientX: number,
    which: "in" | "out",
    scrub = false,
  ) {
    const raw = clientXToTime(clientX);
    const t = quantizeToFrame(raw, fps);
    const min = minClipDuration(fps);
    if (which === "in") {
      const next = Math.max(0, Math.min(t, store.outPoint - min));
      store.trimStart = next;
      // Scrub-while-trim: park playback at the in point so the preview
      // shows the first kept frame as the user drags.
      if (scrub) {
        store.currentTime = next;
        if (videoEl) videoEl.currentTime = next;
      }
    } else {
      const next = Math.min(duration, Math.max(t, store.inPoint + min));
      store.trimEnd = next;
      if (scrub) {
        // Show one frame before the cut (the last kept frame) — that's the
        // frame the user is actually deciding to keep or discard.
        const previewAt = Math.max(store.inPoint, next - frameStep(fps));
        store.currentTime = previewAt;
        if (videoEl) videoEl.currentTime = previewAt;
      }
    }
  }

  function nudgeTrimByKey(which: "in" | "out", direction: 1 | -1, second: boolean) {
    if (duration <= 0) return;
    store.pushUndoStateCoalesced(`trim-${which}`, 500);
    const delta = direction * (second ? 1 : frameStep(fps));
    const min = minClipDuration(fps);
    if (which === "in") {
      const next = quantizeToFrame(
        Math.max(0, Math.min(store.outPoint - min, store.inPoint + delta)),
        fps,
      );
      store.trimStart = next;
    } else {
      const next = quantizeToFrame(
        Math.max(store.inPoint + min, Math.min(duration, store.outPoint + delta)),
        fps,
      );
      store.trimEnd = next;
    }
  }

  function handleTrimHandleKey(event: KeyboardEvent, which: "in" | "out") {
    if (duration <= 0) return;
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();
    nudgeTrimByKey(which, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey);
  }
</script>

<div class="relative h-12 rounded-md border border-border/60 bg-background">
  <span
    class="pointer-events-none sticky left-1.5 top-1 z-50 inline-flex w-fit items-center rounded-sm bg-foreground/10 px-1.5 py-px font-mono text-[8px] font-bold uppercase tracking-wider text-foreground/80 backdrop-blur-sm"
  >
    Clip
  </span>
  <div
    class="absolute inset-y-0 rounded-md border border-primary/40 bg-primary/5"
    style="left: {clipLeft}px; width: {clipWidth}px;"
  >
    <div class="absolute inset-0 overflow-hidden rounded-md">
      {#if store.thumbnailStrip.length > 0}
        <div class="flex h-full">
          {#each store.thumbnailStrip as frame, index (frame + index)}
            <img
              in:fade={{ duration: 180 }}
              src={frame}
              alt="Timeline frame"
              class="h-full shrink-0 object-cover"
              style="width: {thumbnailWidth}px;"
              draggable="false"
            />
          {/each}
        </div>
      {:else}
        <div
          class="flex h-full items-center justify-center text-[10px] text-muted-foreground"
        >
          Generating thumbnails…
        </div>
      {/if}
    </div>

    <!-- Export-status badge. Anchored top-right so it doesn't fight the
         outer "Clip" lane label, which sits top-left. -->
    <div
      class="absolute right-2 top-1 rounded border border-border bg-background/80 px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground backdrop-blur"
    >
      {hasTrim ? "This part exports" : "Full clip"}
    </div>

    <!--
      Trim drag handles. Each is a narrow vertical bar with a larger
      invisible hit area (±6 px either side) so grabbing is easy.
      Pointer events stop propagation so we don't fight the timeline's
      click-to-seek / playhead-scrub handlers.
    -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      role="slider"
      tabindex="0"
      aria-label="In point"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={store.inPoint}
      aria-valuetext={formatTimecode(store.inPoint, fps)}
      onpointerdown={(e) => startTrimDrag(e, "in")}
      onkeydown={(e) => handleTrimHandleKey(e, "in")}
      class="group absolute inset-y-0 left-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
    >
      <div
        class="mx-auto h-full w-1 rounded-l-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"
      ></div>
      {#if activeTrimHandle === "in" && trimDragContext}
        {@const delta = store.inPoint - trimDragContext.originalAt}
        <div
          class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
        >
          <span>In {formatTimeByMode(store.inPoint, timeMode, fps)}</span>
          {#if delta !== 0}
            <span class="text-muted-foreground"
              >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
            >
          {/if}
        </div>
      {/if}
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      role="slider"
      tabindex="0"
      aria-label="Out point"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={store.outPoint}
      aria-valuetext={formatTimecode(store.outPoint, fps)}
      onpointerdown={(e) => startTrimDrag(e, "out")}
      onkeydown={(e) => handleTrimHandleKey(e, "out")}
      class="group absolute inset-y-0 right-0 z-10 w-2 translate-x-1 cursor-ew-resize focus-visible:outline-none"
    >
      <div
        class="mx-auto h-full w-1 rounded-r-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"
      ></div>
      {#if activeTrimHandle === "out" && trimDragContext}
        {@const delta = store.outPoint - trimDragContext.originalAt}
        <div
          class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
        >
          <span>Out {formatTimeByMode(store.outPoint, timeMode, fps)}</span>
          {#if delta !== 0}
            <span class="text-muted-foreground"
              >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
            >
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>
