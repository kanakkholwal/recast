<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { Plus, ZoomIn } from "@recast/icons";
  import type { TimeMode } from "./timeline-helpers";
  import { buildSnapTargets, snapLabel, type SnapTarget } from "./timeline-snap";
  import {
    cardSpan,
    laneHeight,
    packRows,
    rowTop,
    ZOOM_ROW_HEIGHT_PX,
  } from "./timeline-stack";
  import ZoomLayerCard from "./ZoomLayerCard.svelte";

  // Hosts zoom-region cards: builds the shared snap target list and paints
  // the guide line when a card reports an active snap during drag/resize.
  //
  // Drag on empty lane space to create a region, the same gesture the Cuts lane
  // uses. The two lanes look identical, so they must behave identically: this one
  // used to do nothing on drag and force a trip to the toolbar instead.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    fps: number;
    duration: number;
    timeMode: TimeMode;
    onCopy: (region: import("$lib/stores/editor-store.svelte").ZoomRegion) => void;
    onDuplicate: (region: import("$lib/stores/editor-store.svelte").ZoomRegion) => void;
  }

  let {
    store,
    pixelsPerSecond,
    fps,
    duration,
    timeMode,
    onCopy,
    onDuplicate,
  }: Props = $props();

  // Matches the toolbar's "Zoom" button, so a region reads the same however it
  // was made.
  const DEFAULT_SCALE = 1.8;
  // Below this a drag is a stray click, not a deliberate region.
  const MIN_REGION = 0.3;

  let laneEl = $state<HTMLDivElement | null>(null);

  // Last writer wins, only one card drags at a time.
  let activeSnap = $state<SnapTarget | null>(null);
  // Snap targets are original times; place the guide on the output axis.
  const snapX = $derived(
    activeSnap
      ? originalToOutput(store.renderMap, activeSnap.time) * pixelsPerSecond
      : 0,
  );

  function targetsFor(excludeId: string): SnapTarget[] {
    return buildSnapTargets({
      playhead: store.currentTime,
      inPoint: store.inPoint,
      outPoint: store.outPoint,
      duration,
      regions: store.zoomRegions,
      annotations: store.annotations,
      excludeRegionId: excludeId,
    });
  }

  interface DragState {
    pointerId: number;
    anchor: number;
    /** null until the drag is long enough to have spawned a region. */
    id: string | null;
  }
  let drag = $state<DragState | null>(null);

  function timeAt(clientX: number): number {
    if (!laneEl) return 0;
    const x = clientX - laneEl.getBoundingClientRect().left;
    // Pointer is in OUTPUT pixels -> output seconds -> original time.
    return Math.min(
      duration,
      Math.max(
        0,
        outputToOriginal(store.renderMap, x / pixelsPerSecond),
      ),
    );
  }

  function onLaneDown(e: PointerEvent) {
    // Only the bare lane background starts a create-drag; cards stop propagation
    // in their own handlers.
    if (e.target !== laneEl || duration <= 0 || e.button !== 0) return;
    // The razor tool owns clicks timeline-wide: let this bubble to the scroller.
    if (store.timelineTool === "razor") return;
    // Bypassed track: refuse the edit rather than create a region that silently
    // wouldn't apply. The inline hint says why.
    if (!store.focusEnabled) return;
    // Stop the timeline's scrub handler from also claiming this drag.
    e.preventDefault();
    e.stopPropagation();
    drag = { pointerId: e.pointerId, anchor: timeAt(e.clientX), id: null };
    laneEl?.setPointerCapture(e.pointerId);
  }

  function onLaneMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const t = timeAt(e.clientX);
    const lo = Math.min(drag.anchor, t);
    const hi = Math.max(drag.anchor, t);
    if (drag.id === null) {
      if (hi - lo < MIN_REGION) return; // not a deliberate drag yet
      drag.id = store.addZoomRegion(lo, hi, DEFAULT_SCALE);
    } else {
      store.updateZoomRegion(drag.id, { start: lo, end: hi });
    }
  }

  function onLaneUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    laneEl?.releasePointerCapture(e.pointerId);
    drag = null;
  }

  // Overlapping regions stacked on top of each other at top: 50%, so the covered
  // one couldn't be clicked or resized. The FocusPanel warns about overlaps, so
  // they're an expected state the lane has to be able to show.
  const MIN_CARD_PX = 32;
  const spans = $derived(
    store.zoomRegions.map((r) => {
      const s = cardSpan(
        originalToOutput(store.renderMap, r.start) * pixelsPerSecond,
        originalToOutput(store.renderMap, r.end) * pixelsPerSecond,
        MIN_CARD_PX,
      );
      return { id: r.id, left: s.left, right: s.left + s.width, width: s.width };
    }),
  );
  const rows = $derived(packRows(spans));
  const height = $derived(
    laneHeight(rows.length ? Math.max(...rows) + 1 : 0, ZOOM_ROW_HEIGHT_PX),
  );

  // Empty-state affordance: the lane used to explain how to add a region without
  // letting you do it. Punches in around the playhead, like the toolbar button.
  function addAtPlayhead() {
    if (duration <= 0 || !store.focusEnabled) return;
    const start = Math.max(store.inPoint, store.currentTime - 0.35);
    const end = Math.min(
      store.outPoint,
      Math.max(start + 0.8, store.currentTime + 0.85),
    );
    store.addZoomRegion(start, end, DEFAULT_SCALE);
  }
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointerdown={onLaneDown}
  onpointermove={onLaneMove}
  onpointerup={onLaneUp}
  onpointercancel={onLaneUp}
  class="relative mt-1.5 cursor-crosshair rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-[height]"
  style="height: {height}px;"
>
  {#if store.zoomRegions.length === 0}
    <button
      type="button"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={addAtPlayhead}
      disabled={duration <= 0}
      class="flex h-6 w-full items-center justify-center gap-1 rounded text-[10px] text-muted-foreground transition-colors hover:bg-lane-zoom/5 hover:text-foreground disabled:opacity-50"
    >
      <Plus class="size-3" />
      Drag here to add a zoom, or click to punch in at the playhead
    </button>
  {:else}
    {#each store.zoomRegions as region, i (region.id)}
      <ZoomLayerCard
        {store}
        {region}
        {pixelsPerSecond}
        {fps}
        {duration}
        left={spans[i].left}
        width={spans[i].width}
        top={rowTop(rows[i], ZOOM_ROW_HEIGHT_PX)}
        snapTargets={targetsFor(region.id)}
        {timeMode}
        onSnapChange={(snap) => (activeSnap = snap)}
        {onCopy}
        {onDuplicate}
      />
    {/each}
  {/if}

  {#if !store.focusEnabled}
    <!-- Bypassed: say why editing is refused rather than dimming silently. -->
    <div
      class="pointer-events-none absolute inset-0 z-30 flex items-center justify-center gap-1.5 rounded-md bg-background/60 text-[10px] font-medium text-foreground"
    >
      <ZoomIn class="size-3 text-lane-zoom" />
      Zoom is off. Turn on "Apply zoom" in Layers to edit.
    </div>
  {/if}

  {#if activeSnap}
    <!-- Drawn tall with negative offsets so the guide crosses the clip bar above too. -->
    <div
      class="pointer-events-none absolute -top-14 z-40 h-42.5 w-px bg-lane-zoom/80"
      style="left: {snapX + 6}px;"
    ></div>
    <div
      class="pointer-events-none absolute -top-14 z-40 -translate-x-1/2 rounded border border-lane-zoom/60 bg-lane-zoom px-1 py-0.5 font-mono text-[9px] text-background shadow-craft-sm"
      style="left: {snapX + 6}px;"
    >
      {snapLabel(activeSnap.kind)}
    </div>
  {/if}
</div>
