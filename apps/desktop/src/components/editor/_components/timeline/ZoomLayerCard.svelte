<script lang="ts">
  import type { EditorStore, ZoomRegion } from "$lib/stores/editor-store.svelte";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { motionDuration } from "$lib/motion.svelte";
  import { X, ZoomIn } from "@recast/icons";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import {
    computeCardMove,
    computeCardNudge,
    computeCardResize,
  } from "./timeline-card-drag.logic";
  import { formatTimeByMode, type TimeMode } from "./timeline-helpers";
  import { type SnapResult, type SnapTarget } from "./timeline-snap";

  // Three drag modes through one pointer-handler: move (shift both edges),
  // resize-start (move `start`), resize-end (move `end`).
  // pushUndoState() fires once at pointer-down so the whole gesture is one undo entry.

  interface Props {
    store: EditorStore;
    region: ZoomRegion;
    pixelsPerSecond: number;
    fps: number;
    duration: number;
    snapTargets: SnapTarget[];
    timeMode: TimeMode;
    onSnapChange: (snap: SnapResult["target"] | null) => void;
    onCopy: (region: ZoomRegion) => void;
    onDuplicate: (region: ZoomRegion) => void;
  }

  let {
    store,
    region,
    pixelsPerSecond,
    fps,
    duration,
    snapTargets,
    timeMode,
    onSnapChange,
    onCopy,
    onDuplicate,
  }: Props = $props();

  // Floor so a card can't collapse to zero width (0.1s ≈ 6 frames at 60fps).
  const MIN_DURATION = 0.1;

  const SNAP_TOLERANCE_PX = 6;

  type DragMode = "move" | "resize-start" | "resize-end";

  interface DragContext {
    mode: DragMode;
    pointerId: number;
    startClientX: number;
    originalStart: number;
    originalEnd: number;
  }

  let drag = $state<DragContext | null>(null);

  const isSelected = $derived(region.id === store.selectedZoomRegionId);
  // Output (post-cut) axis so regions sit on the same gapless line as clips;
  // a region overlapping a cut renders narrower (correct NLE behaviour).
  const xOf = (t: number) =>
    originalToOutput(store.renderMap, t) * pixelsPerSecond;
  const tOf = (xPx: number) =>
    outputToOriginal(store.renderMap, xPx / pixelsPerSecond);
  // Labels read on the output axis, like the ruler and the playhead. Regions are
  // STORED in original time, so printing that raw would name a timecode the
  // exported file never reaches once anything upstream is cut.
  const outSec = (t: number) => originalToOutput(store.renderMap, t);
  const left = $derived(xOf(region.start));
  // 32px floor keeps even sub-frame regions clickable.
  const width = $derived(Math.max(xOf(region.end) - xOf(region.start), 32));
  const showSubtitle = $derived(width >= 110);

  function beginDrag(mode: DragMode, event: PointerEvent) {
    if (duration <= 0) return;
    // Let a razor click bubble through to carve, rather than dragging the card.
    if (store.timelineTool === "razor") return;
    event.preventDefault();
    event.stopPropagation();
    store.selectedZoomRegionId = region.id;
    store.pushUndoState();
    drag = {
      mode,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      originalStart: region.start,
      originalEnd: region.end,
    };
    document.body.style.cursor =
      mode === "move" ? "grabbing" : "ew-resize";
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  }

  function onPointerMove(event: PointerEvent) {
    if (!drag) return;
    const geom = {
      origin: { start: drag.originalStart, end: drag.originalEnd },
      clientX: event.clientX,
      startClientX: drag.startClientX,
      pps: pixelsPerSecond,
      xOf,
      tOf,
      snapTargets,
      tolerance: SNAP_TOLERANCE_PX / pixelsPerSecond,
      fps,
      duration,
    };
    const result =
      drag.mode === "move"
        ? computeCardMove(geom)
        : computeCardResize({
            ...geom,
            edge: drag.mode === "resize-start" ? "start" : "end",
            minDuration: MIN_DURATION,
          });
    store.updateZoomRegion(region.id, { start: result.start, end: result.end });
    onSnapChange(result.guide);
  }

  function onPointerUp(_event: PointerEvent) {
    drag = null;
    document.body.style.cursor = "";
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerUp);
    onSnapChange(null);
  }

  // Coalesces sequential nudges into one undo entry so a held arrow is one edit.
  function onCardKeydown(event: KeyboardEvent) {
    if (duration <= 0) return;

    // Delete is owned by the editor page and acts on the selection (this card is
    // the selection whenever it has focus), so it is deliberately not handled here.

    // Paste lives at timeline scope so regions land at the playhead, not here.
    const isMod = event.ctrlKey || event.metaKey;
    if (isMod && (event.key === "d" || event.key === "D")) {
      event.preventDefault();
      event.stopPropagation();
      onDuplicate(region);
      return;
    }
    if (isMod && (event.key === "c" || event.key === "C")) {
      event.preventDefault();
      event.stopPropagation();
      onCopy(region);
      return;
    }

    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();

    store.pushUndoStateCoalesced(`nudge-zoom-${region.id}`, 600);

    const next = computeCardNudge({
      origin: { start: region.start, end: region.end },
      direction: event.key === "ArrowLeft" ? -1 : 1,
      shift: event.shiftKey,
      alt: event.altKey,
      fps,
      duration,
      minDuration: MIN_DURATION,
    });
    store.updateZoomRegion(region.id, { start: next.start, end: next.end });
  }

  function onCardClick(event: MouseEvent) {
    // A real drag never fires this (window-level pointer handlers); only a static click does.
    if (store.timelineTool === "razor") return; // razor click is not a select
    event.stopPropagation();
    store.selectedZoomRegionId = region.id;
  }

  function onRemove(event: Event) {
    event.stopPropagation();
    if (event instanceof KeyboardEvent) {
      event.preventDefault();
      if (event.key !== "Enter" && event.key !== " ") return;
    }
    store.removeZoomRegion(region.id);
  }
</script>

<div
  in:fly={{ y: 10, duration: motionDuration(180), easing: cubicOut }}
  out:fade={{ duration: motionDuration(140) }}
  class="group/card absolute z-20 overflow-visible select-none"
  style="
    left: {left}px;
    width: {width}px;
    top: 50%;
    margin-top: -15px;
    height: 30px;
  "
>
  <!-- Body split from the resize edges so each gets its own cursor; selected state
       uses a box-shadow inset accent bar to avoid layout shift. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <button
    type="button"
    aria-pressed={isSelected}
    onclick={onCardClick}
    onkeydown={onCardKeydown}
    onpointerdown={(e) => {
      // Resize handlers stop propagation, so anything reaching here is a body drag.
      if (e.button !== 0) return;
      beginDrag("move", e);
    }}
    class="absolute inset-0 overflow-hidden rounded-md border bg-lane-zoom/10 text-left backdrop-blur-sm transition-all duration-150 hover:bg-lane-zoom/20 hover:shadow-craft-sm focus:outline-none focus:ring-1 focus:ring-ring {isSelected
      ? 'border-lane-zoom cursor-grabbing shadow-[inset_3px_0_0_0_var(--color-lane-zoom)] hover:shadow-[inset_3px_0_0_0_var(--color-lane-zoom)]'
      : 'border-lane-zoom/30 hover:border-lane-zoom/60 cursor-grab'} {drag?.mode === 'move'
      ? 'cursor-grabbing shadow-craft-floating'
      : ''}"
  >
    <div
      class="relative flex h-full items-center gap-1.5 px-1.5"
      id={`zoom-region-${region.id}`}
      aria-label={`Focus region from ${formatTimeByMode(outSec(region.start), timeMode, fps)} to ${formatTimeByMode(outSec(region.end), timeMode, fps)}, scale ${region.scale.toFixed(1)}x. Click to select; drag to move; drag the edges to resize.`}
    >
      <span
        class="flex size-5 shrink-0 items-center justify-center rounded-md bg-lane-zoom/20 text-lane-zoom"
      >
        <ZoomIn class="size-3" />
      </span>
      <div class="min-w-0 flex-1 pointer-events-none">
        <p class="truncate text-[10px] font-semibold leading-tight text-foreground">
          Zoom <span class="text-lane-zoom">{region.scale.toFixed(1)}×</span>
        </p>
        {#if showSubtitle}
          <p
            class="truncate text-[9px] leading-tight tabular-nums text-muted-foreground"
          >
            {formatTimeByMode(outSec(region.start), timeMode, fps)}
          </p>
        {/if}
      </div>
      <span
        role="button"
        id={`remove-zoom-region-${region.id}`}
        tabindex="0"
        onclick={onRemove}
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={onRemove}
        class="pointer-events-auto flex size-4 shrink-0 cursor-pointer items-center justify-center rounded border border-border bg-background/70 text-muted-foreground opacity-0 transition-all hover:border-destructive hover:text-destructive group-hover/card:opacity-100 focus:opacity-100 {isSelected
          ? 'opacity-100'
          : ''}"
        aria-label="Remove focus region"
      >
        <X size={9} stroke={2.5} />
      </span>
    </div>
  </button>

  <!-- Resize handles: 8px hit zone above the body so pointer events land here first. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="-1"
    aria-label="Resize region start"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={region.start}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-start", e);
    }}
    class="absolute inset-y-0 left-0 z-10 w-2 cursor-ew-resize"
  >
    <div
      class="mx-auto h-full w-0.5 rounded-l-sm bg-lane-zoom/70 opacity-0 transition-opacity group-hover:opacity-100 {isSelected ||
      drag?.mode === 'resize-start'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="-1"
    aria-label="Resize region end"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={region.end}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-end", e);
    }}
    class="absolute inset-y-0 right-0 z-10 w-2 cursor-ew-resize"
  >
    <div
      class="ml-auto h-full w-0.5 rounded-r-sm bg-lane-zoom/70 opacity-0 transition-opacity group-hover:opacity-100 {isSelected ||
      drag?.mode === 'resize-end'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
</div>
