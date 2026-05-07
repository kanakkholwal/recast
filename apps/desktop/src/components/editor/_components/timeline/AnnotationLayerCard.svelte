<script lang="ts">
  import { kindIcon, kindLabel } from "$lib/annotations/kind-label";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { X } from "@lucide/svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import {
    formatTimeByMode,
    frameStep,
    type TimeMode,
  } from "./timeline-helpers";
  import { snapTime, type SnapResult, type SnapTarget } from "./timeline-snap";

  // Annotation timeline card. Mirrors ZoomLayerCard's drag/resize/snap
  // behaviour but operates on annotations (visible via store.annotations,
  // mutated through store.updateAnnotation). Visual treatment is
  // deliberately distinct — outline only, no sparkline — so the two lanes
  // are distinguishable at a glance.

  interface Props {
    store: EditorStore;
    annotation: Annotation;
    index: number;
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
    index,
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
  const left = $derived(annotation.start * pixelsPerSecond);
  // 28px lets a single icon stay grabbable even on a one-frame annotation.
  // Subtitle (start time) appears once the card is wide enough to fit it
  // alongside the kind label.
  const width = $derived(
    Math.max((annotation.end - annotation.start) * pixelsPerSecond, 28),
  );
  const showSubtitle = $derived(width >= 110);
  const Icon = $derived(kindIcon(annotation));

  function beginDrag(mode: DragMode, event: PointerEvent) {
    if (duration <= 0) return;
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
    const deltaTime = (event.clientX - drag.startClientX) / pixelsPerSecond;
    const tolerance = SNAP_TOLERANCE_PX / pixelsPerSecond;
    let snapForGuide: SnapTarget | null = null;

    if (drag.mode === "move") {
      const span = drag.originalEnd - drag.originalStart;
      const proposed = drag.originalStart + deltaTime;
      const startSnap = snapTime(proposed, snapTargets, tolerance, fps);
      const endSnap = snapTime(proposed + span, snapTargets, tolerance, fps);
      const startDist = startSnap.target
        ? Math.abs(startSnap.time - proposed)
        : Infinity;
      const endDist = endSnap.target
        ? Math.abs(endSnap.time - (proposed + span))
        : Infinity;

      let nextStart: number;
      if (startSnap.target && startDist <= endDist) {
        nextStart = startSnap.time;
        snapForGuide = startSnap.target;
      } else if (endSnap.target) {
        nextStart = endSnap.time - span;
        snapForGuide = endSnap.target;
      } else {
        nextStart = startSnap.time;
      }

      nextStart = Math.max(0, Math.min(duration - span, nextStart));
      const nextEnd = nextStart + span;
      store.updateAnnotation(annotation.id, { start: nextStart, end: nextEnd });
    } else if (drag.mode === "resize-start") {
      const proposed = drag.originalStart + deltaTime;
      const snap = snapTime(proposed, snapTargets, tolerance, fps);
      snapForGuide = snap.target;
      const next = Math.max(
        0,
        Math.min(drag.originalEnd - MIN_DURATION, snap.time),
      );
      store.updateAnnotation(annotation.id, { start: next });
    } else {
      const proposed = drag.originalEnd + deltaTime;
      const snap = snapTime(proposed, snapTargets, tolerance, fps);
      snapForGuide = snap.target;
      const next = Math.min(
        duration,
        Math.max(drag.originalStart + MIN_DURATION, snap.time),
      );
      store.updateAnnotation(annotation.id, { end: next });
    }

    onSnapChange(snapForGuide);
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

    const direction = event.key === "ArrowLeft" ? -1 : 1;
    const delta = direction * (event.shiftKey ? 1 : frameStep(fps));

    store.pushUndoStateCoalesced(`nudge-annotation-${annotation.id}`, 600);

    if (event.altKey) {
      const next = Math.min(
        duration,
        Math.max(annotation.start + MIN_DURATION, annotation.end + delta),
      );
      store.updateAnnotation(annotation.id, { end: next });
      return;
    }

    const span = annotation.end - annotation.start;
    let nextStart = annotation.start + delta;
    nextStart = Math.max(0, Math.min(duration - span, nextStart));
    store.updateAnnotation(annotation.id, {
      start: nextStart,
      end: nextStart + span,
    });
  }

  function onCardClick(event: MouseEvent) {
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
  in:fly={{ y: 10, duration: 180, easing: cubicOut }}
  out:fade={{ duration: 140 }}
  class="group/card absolute z-20 overflow-visible select-none"
  style="
    left: {left}px;
    width: {width}px;
    top: {2 + index * 2}px;
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
    class="absolute inset-0 overflow-hidden rounded border bg-card/85 text-left backdrop-blur-sm transition-all duration-150 hover:bg-card hover:shadow-craft-sm focus:outline-none focus:ring-1 focus:ring-ring {isSelected
      ? 'border-amber-500/80 cursor-grabbing shadow-[inset_3px_0_0_0_rgba(245,158,11,0.9)] hover:shadow-[inset_3px_0_0_0_rgba(245,158,11,0.9)]'
      : 'border-amber-500/40 hover:border-amber-500/70 cursor-grab'} {drag?.mode ===
    'move'
      ? 'cursor-grabbing shadow-craft-floating'
      : ''}"
  >
    <div
      class="relative flex h-full items-center gap-1.5 px-1.5"
      id={`annotation-region-${annotation.id}`}
      aria-label={`${kindLabel(annotation)} annotation from ${formatTimeByMode(annotation.start, timeMode, fps)} to ${formatTimeByMode(annotation.end, timeMode, fps)}. Click to select; drag to move; drag the edges to resize.`}
    >
      <span
        class="flex size-4 shrink-0 items-center justify-center rounded text-amber-600 dark:text-amber-400"
      >
        <Icon class="size-3" />
      </span>
      <div class="min-w-0 flex-1 pointer-events-none">
        <p class="truncate text-[10px] font-semibold leading-tight text-foreground">
          {kindLabel(annotation)}
        </p>
        {#if showSubtitle}
          <p class="truncate text-[9px] leading-tight text-muted-foreground">
            {formatTimeByMode(annotation.start, timeMode, fps)}
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
        <X size={9} strokeWidth={2.5} />
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
      class="mx-auto h-full w-0.5 rounded-l-sm bg-amber-500/70 opacity-0 transition-opacity {isSelected ||
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
      class="ml-auto h-full w-0.5 rounded-r-sm bg-amber-500/70 opacity-0 transition-opacity {isSelected ||
      drag?.mode === 'resize-end'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
</div>
