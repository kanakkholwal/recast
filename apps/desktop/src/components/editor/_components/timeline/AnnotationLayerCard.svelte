<script lang="ts">
  import { kindIcon, kindLabel } from "$lib/annotations/kind-label";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { motionDuration } from "$lib/motion.svelte";
  import { X } from "@recast/icons";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import {
    computeCardMove,
    computeCardNudge,
    computeCardResize,
  } from "./timeline-card-drag.logic";
  import { formatTimeByMode, type TimeMode } from "./timeline-helpers";
  import { type SnapResult, type SnapTarget } from "./timeline-snap";

  // Mirrors ZoomLayerCard's drag/resize/snap on annotations; outline-only
  // (no sparkline) so the two lanes are distinguishable at a glance.

  interface Props {
    store: EditorStore;
    annotation: Annotation;
    pixelsPerSecond: number;
    fps: number;
    duration: number;
    snapTargets: SnapTarget[];
    timeMode: TimeMode;
    onSnapChange: (snap: SnapResult["target"] | null) => void;
    onDuplicate: (annotation: Annotation) => void;
  }

  let {
    store,
    annotation,
    pixelsPerSecond,
    fps,
    duration,
    snapTargets,
    timeMode,
    onSnapChange,
    onDuplicate,
  }: Props = $props();

  const MIN_DURATION = 0.05; // Annotations can be tighter than zooms.
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

  const isSelected = $derived(annotation.id === store.selectedAnnotationId);
  // Output (post-cut) axis. See ZoomLayerCard for the rationale.
  const xOf = (t: number) =>
    originalToOutput(store.timeMap, t) * pixelsPerSecond;
  const tOf = (xPx: number) =>
    outputToOriginal(store.timeMap, xPx / pixelsPerSecond);
  // Labels read on the output axis, like the ruler and the playhead.
  const outSec = (t: number) => originalToOutput(store.timeMap, t);
  const left = $derived(xOf(annotation.start));
  // 28px keeps a one-frame annotation grabbable.
  const width = $derived(
    Math.max(xOf(annotation.end) - xOf(annotation.start), 28),
  );
  const showSubtitle = $derived(width >= 110);
  const Icon = $derived(kindIcon(annotation));

  function beginDrag(mode: DragMode, event: PointerEvent) {
    if (duration <= 0) return;
    // Let a razor click bubble through to carve, rather than dragging the card.
    if (store.timelineTool === "razor") return;
    event.preventDefault();
    event.stopPropagation();
    store.selectedAnnotationId = annotation.id;
    store.pushUndoState();
    drag = {
      mode,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      originalStart: annotation.start,
      originalEnd: annotation.end,
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
    store.updateAnnotation(annotation.id, {
      start: result.start,
      end: result.end,
    });
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

  function onCardKeydown(event: KeyboardEvent) {
    if (duration <= 0) return;

    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      event.stopPropagation();
      store.removeAnnotation(annotation.id);
      return;
    }

    const isMod = event.ctrlKey || event.metaKey;
    if (isMod && (event.key === "d" || event.key === "D")) {
      event.preventDefault();
      event.stopPropagation();
      onDuplicate(annotation);
      return;
    }

    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();

    store.pushUndoStateCoalesced(`nudge-annotation-${annotation.id}`, 600);

    const next = computeCardNudge({
      origin: { start: annotation.start, end: annotation.end },
      direction: event.key === "ArrowLeft" ? -1 : 1,
      shift: event.shiftKey,
      alt: event.altKey,
      fps,
      duration,
      minDuration: MIN_DURATION,
    });
    store.updateAnnotation(annotation.id, {
      start: next.start,
      end: next.end,
    });
  }

  function onCardClick(event: MouseEvent) {
    if (store.timelineTool === "razor") return; // razor click is not a select
    event.stopPropagation();
    store.selectedAnnotationId = annotation.id;
  }

  function onRemove(event: Event) {
    event.stopPropagation();
    if (event instanceof KeyboardEvent) {
      event.preventDefault();
      if (event.key !== "Enter" && event.key !== " ") return;
    }
    store.removeAnnotation(annotation.id);
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
    margin-top: -13px;
    height: 26px;
  "
>
  <button
    type="button"
    aria-pressed={isSelected}
    onclick={onCardClick}
    onkeydown={onCardKeydown}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("move", e);
    }}
    class="absolute inset-0 overflow-hidden rounded-md border bg-lane-markup/10 text-left backdrop-blur-sm transition-all duration-150 hover:bg-lane-markup/20 hover:shadow-craft-sm focus:outline-none focus:ring-1 focus:ring-ring {isSelected
      ? 'border-lane-markup/80 cursor-grabbing shadow-[inset_3px_0_0_0_var(--color-lane-markup)] hover:shadow-[inset_3px_0_0_0_var(--color-lane-markup)]'
      : 'border-lane-markup/40 hover:border-lane-markup/70 cursor-grab'} {drag?.mode ===
    'move'
      ? 'cursor-grabbing shadow-craft-floating'
      : ''}"
  >
    <div
      class="relative flex h-full items-center gap-1.5 px-1.5"
      id={`annotation-region-${annotation.id}`}
      aria-label={`${kindLabel(annotation)} annotation from ${formatTimeByMode(outSec(annotation.start), timeMode, fps)} to ${formatTimeByMode(outSec(annotation.end), timeMode, fps)}. Click to select; drag to move; drag the edges to resize.`}
    >
      <span
        class="flex size-5 shrink-0 items-center justify-center rounded-md bg-lane-markup/20 text-lane-markup"
      >
        <Icon class="size-3" />
      </span>
      <div class="min-w-0 flex-1 pointer-events-none">
        <p class="truncate text-[10px] font-semibold leading-tight text-foreground">
          {kindLabel(annotation)}
        </p>
        {#if showSubtitle}
          <p class="truncate text-[9px] leading-tight text-muted-foreground">
            {formatTimeByMode(outSec(annotation.start), timeMode, fps)}
          </p>
        {/if}
      </div>
      <span
        role="button"
        tabindex="0"
        onclick={onRemove}
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={onRemove}
        class="pointer-events-auto flex size-4 shrink-0 cursor-pointer items-center justify-center rounded border border-border bg-background/70 text-muted-foreground opacity-0 transition-all hover:border-destructive hover:text-destructive group-hover/card:opacity-100 focus:opacity-100 {isSelected
          ? 'opacity-100'
          : ''}"
        aria-label="Remove annotation"
      >
        <X size={9} stroke={2.5} />
      </span>
    </div>
  </button>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="-1"
    aria-label="Resize annotation start"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={annotation.start}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-start", e);
    }}
    class="absolute inset-y-0 left-0 z-10 w-2 cursor-ew-resize"
  >
    <div
      class="mx-auto h-full w-0.5 rounded-l-sm bg-lane-markup/70 opacity-0 transition-opacity {isSelected ||
      drag?.mode === 'resize-start'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="-1"
    aria-label="Resize annotation end"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={annotation.end}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-end", e);
    }}
    class="absolute inset-y-0 right-0 z-10 w-2 cursor-ew-resize"
  >
    <div
      class="ml-auto h-full w-0.5 rounded-r-sm bg-lane-markup/70 opacity-0 transition-opacity {isSelected ||
      drag?.mode === 'resize-end'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
</div>
