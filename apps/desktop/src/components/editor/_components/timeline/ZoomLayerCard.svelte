<script lang="ts">
  import type { EditorStore, ZoomRegion } from "$lib/stores/editor-store.svelte";
  import { X } from "@lucide/svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import {
    formatTimeByMode,
    frameStep,
    zoomSparklinePath,
    type TimeMode,
  } from "./timeline-helpers";
  import { snapTime, type SnapResult, type SnapTarget } from "./timeline-snap";

  // Single zoom-region card. Three drag modes share one pointer-handler
  // because they all read the same pixels-per-second translation:
  //   - body drag      → translate the whole region (start AND end shift)
  //   - resize-start   → only `start` moves, `end` stays
  //   - resize-end     → only `end` moves, `start` stays
  //
  // A drag captures `pushUndoState()` once at pointer-down so the entire
  // gesture collapses to a single undo entry, then commits on every move.
  // Snap targets come from the lane (playhead, in/out, neighbours).

  interface Props {
    store: EditorStore;
    region: ZoomRegion;
    index: number;
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
    index,
    pixelsPerSecond,
    fps,
    duration,
    snapTargets,
    timeMode,
    onSnapChange,
    onCopy,
    onDuplicate,
  }: Props = $props();

  // Pick a frame from the clip-wide thumbnail strip closest to this region's
  // start. Cheap visual identifier — the strip is already loaded for the
  // clip bar, so this is a free reuse (no extra ffmpeg call).
  const cardThumb = $derived.by(() => {
    const strip = store.thumbnailStrip;
    if (!strip.length || duration <= 0) return null;
    const idx = Math.min(
      strip.length - 1,
      Math.max(0, Math.floor((region.start / duration) * strip.length)),
    );
    return strip[idx] ?? null;
  });

  // Hard floor so a card can't be dragged into a degenerate zero-width state.
  // 0.1s ≈ 6 frames at 60fps — still tight enough to be intentional.
  const MIN_DURATION = 0.1;

  // ~6 px in time-space. The lane re-renders the snap guide whenever the
  // active target changes, so this also drives the user-visible feedback.
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
  const left = $derived(region.start * pixelsPerSecond);
  // Hard floor of 32px keeps even sub-frame regions clickable. Below 80px
  // the card collapses to icon+label only; below 56px to icon only.
  const width = $derived(
    Math.max((region.end - region.start) * pixelsPerSecond, 32),
  );
  const showThumb = $derived(width >= 110);
  const showSubtitle = $derived(width >= 130);

  function beginDrag(mode: DragMode, event: PointerEvent) {
    if (duration <= 0) return;
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
    const deltaTime = (event.clientX - drag.startClientX) / pixelsPerSecond;
    const tolerance = SNAP_TOLERANCE_PX / pixelsPerSecond;
    let snapForGuide: SnapTarget | null = null;

    if (drag.mode === "move") {
      const span = drag.originalEnd - drag.originalStart;
      const proposed = drag.originalStart + deltaTime;

      // Snap whichever edge is closer to a target — this lets the user
      // butt the card up against the playhead from either side without
      // having to think about which edge is leading.
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
        nextStart = startSnap.time; // frame-quantised fallback
      }

      // Clamp inside [0, duration] without changing span.
      nextStart = Math.max(0, Math.min(duration - span, nextStart));
      const nextEnd = nextStart + span;
      store.updateZoomRegion(region.id, { start: nextStart, end: nextEnd });
    } else if (drag.mode === "resize-start") {
      const proposed = drag.originalStart + deltaTime;
      const snap = snapTime(proposed, snapTargets, tolerance, fps);
      snapForGuide = snap.target;
      const next = Math.max(
        0,
        Math.min(drag.originalEnd - MIN_DURATION, snap.time),
      );
      store.updateZoomRegion(region.id, { start: next });
    } else {
      const proposed = drag.originalEnd + deltaTime;
      const snap = snapTime(proposed, snapTargets, tolerance, fps);
      snapForGuide = snap.target;
      const next = Math.min(
        duration,
        Math.max(drag.originalStart + MIN_DURATION, snap.time),
      );
      store.updateZoomRegion(region.id, { end: next });
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

  // Keyboard nudge / shortcut handler for the focused card. Coalesces
  // sequential nudges into one undo entry so holding ArrowRight feels like
  // one continuous edit rather than dozens of stack frames.
  function onCardKeydown(event: KeyboardEvent) {
    if (duration <= 0) return;

    // Delete / Backspace removes. Plain key — no modifier required —
    // because the card itself is focused, not a text input.
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      event.stopPropagation();
      store.removeZoomRegion(region.id);
      return;
    }

    // Cmd/Ctrl + D duplicates. Cmd/Ctrl + C copies. Paste happens at the
    // timeline scope so users can land regions wherever the playhead is.
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

    const direction = event.key === "ArrowLeft" ? -1 : 1;
    // Shift = 1 second nudges, plain = single frame. Mirrors the playhead
    // arrow-step convention in Timeline.svelte.
    const delta = direction * (event.shiftKey ? 1 : frameStep(fps));

    store.pushUndoStateCoalesced(`nudge-zoom-${region.id}`, 600);

    // Alt = resize the trailing edge instead of translating the card. Lets
    // users tighten/loosen a region from the keyboard without aiming at
    // the edge handles.
    if (event.altKey) {
      const next = Math.min(
        duration,
        Math.max(region.start + MIN_DURATION, region.end + delta),
      );
      store.updateZoomRegion(region.id, { end: next });
      return;
    }

    const span = region.end - region.start;
    let nextStart = region.start + delta;
    nextStart = Math.max(0, Math.min(duration - span, nextStart));
    store.updateZoomRegion(region.id, {
      start: nextStart,
      end: nextStart + span,
    });
  }

  function onCardClick(event: MouseEvent) {
    // Click selects only when no drag occurred. Pointer handlers live on
    // window so a real drag never fires this — but a static click without
    // motion still reaches us via the body's `onclick`.
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
  in:fly={{ y: 10, duration: 180, easing: cubicOut }}
  out:fade={{ duration: 140 }}
  class="group/card absolute z-20 overflow-visible select-none"
  style="
    left: {left}px;
    width: {width}px;
    top: {2 + index * 2}px;
    height: 30px;
  "
>
  <!-- Card body. We split the visual rectangle from the move hit-target
       so the resize edges can sit on top with their own cursor.
       Selected state earns a 2-px left accent bar via box-shadow inset
       so it reads as "this is the active layer" without jiggling layout. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <button
    type="button"
    aria-pressed={isSelected}
    onclick={onCardClick}
    onkeydown={onCardKeydown}
    onpointerdown={(e) => {
      // Resizing has priority — those handlers stop propagation, so any
      // pointerdown that reaches here is intended as a body drag.
      if (e.button !== 0) return;
      beginDrag("move", e);
    }}
    class="absolute inset-0 overflow-hidden rounded border bg-card/85 text-left backdrop-blur-sm transition-all duration-150 hover:bg-card hover:shadow-craft-sm focus:outline-none focus:ring-1 focus:ring-ring {isSelected
      ? 'border-primary cursor-grabbing shadow-[inset_3px_0_0_0_var(--color-primary)] hover:shadow-[inset_3px_0_0_0_var(--color-primary)]'
      : 'border-border hover:border-primary/50 cursor-grab'} {drag?.mode === 'move'
      ? 'cursor-grabbing shadow-craft-floating'
      : ''}"
  >
    <svg
      viewBox="0 0 100 18"
      preserveAspectRatio="none"
      class="pointer-events-none absolute inset-x-0 bottom-0 h-3 w-full text-primary/60"
    >
      <path
        d={zoomSparklinePath(region)}
        stroke="currentColor"
        stroke-width="1.2"
        fill="none"
      />
    </svg>
    <div
      class="relative flex h-full items-center gap-1.5 px-1.5"
      id={`zoom-region-${region.id}`}
      aria-label={`Focus region from ${formatTimeByMode(region.start, timeMode, fps)} to ${formatTimeByMode(region.end, timeMode, fps)}, scale ${region.scale.toFixed(1)}x. Click to select; drag to move; drag the edges to resize.`}
    >
      {#if showThumb && cardThumb}
        <img
          src={cardThumb}
          alt=""
          aria-hidden="true"
          draggable="false"
          class="pointer-events-none h-6 w-9 shrink-0 rounded-sm border border-border/50 object-cover"
        />
      {/if}
      <div class="min-w-0 flex-1 pointer-events-none">
        <p class="truncate text-[10px] font-semibold leading-tight text-foreground">
          {region.scale.toFixed(1)}× Focus
        </p>
        {#if showSubtitle}
          <p class="truncate text-[9px] leading-tight text-muted-foreground">
            {formatTimeByMode(region.start, timeMode, fps)}
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
        <X size={9} strokeWidth={2.5} />
      </span>
    </div>
  </button>

  <!-- Resize handles. 8px wide hit zone, 2px visible bar at the inner edge.
       z-index above the body so pointer events land here first. -->
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
      class="mx-auto h-full w-0.5 rounded-l-sm bg-primary/70 opacity-0 transition-opacity group-hover:opacity-100 {isSelected ||
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
      class="ml-auto h-full w-0.5 rounded-r-sm bg-primary/70 opacity-0 transition-opacity group-hover:opacity-100 {isSelected ||
      drag?.mode === 'resize-end'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
</div>
