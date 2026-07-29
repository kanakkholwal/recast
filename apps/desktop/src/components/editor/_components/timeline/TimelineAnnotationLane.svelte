<script lang="ts">
  import type { Annotation, EditorStore } from "$lib/stores/editor-store.svelte";
  import { originalToOutput } from "$lib/timeline/time-map";
  import type { TimeMode } from "./timeline-helpers";
  import { buildSnapTargets, snapLabel, type SnapTarget } from "./timeline-snap";
  import { cardSpan, laneHeight, packRows, rowTop } from "./timeline-stack";
  import AnnotationLayerCard from "./AnnotationLayerCard.svelte";

  // Sister of TimelineZoomLane; same lifted snap-guide pattern.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    fps: number;
    duration: number;
    timeMode: TimeMode;
    onDuplicate: (annotation: Annotation) => void;
  }

  let {
    store,
    pixelsPerSecond,
    fps,
    duration,
    timeMode,
    onDuplicate,
  }: Props = $props();

  let activeSnap = $state<SnapTarget | null>(null);
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
      excludeAnnotationId: excludeId,
    });
  }

  // Overlapping annotations are the normal case (a box and its label share a
  // moment), and every card used to sit at top: 50%, so the covered one could
  // not be clicked or resized at all. The lane owns layout now: it measures each
  // card, packs them into rows, and grows to fit.
  const spans = $derived(
    store.annotations.map((a) => {
      const s = cardSpan(
        originalToOutput(store.renderMap, a.start) * pixelsPerSecond,
        originalToOutput(store.renderMap, a.end) * pixelsPerSecond,
      );
      return { id: a.id, left: s.left, right: s.left + s.width, width: s.width };
    }),
  );
  const rows = $derived(packRows(spans));
  const height = $derived(laneHeight(rows.length ? Math.max(...rows) + 1 : 0));
</script>

<div
  class="relative mt-1.5 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-[opacity,height]"
  class:opacity-50={store.annotationsGloballyHidden}
  style="height: {height}px;"
>
  {#if store.annotations.length === 0}
    <div
      class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Annotations you draw on the preview appear here as draggable layers
    </div>
  {:else}
    {#each store.annotations as annotation, i (annotation.id)}
      <AnnotationLayerCard
        {store}
        {annotation}
        {pixelsPerSecond}
        {fps}
        {duration}
        left={spans[i].left}
        width={spans[i].width}
        top={rowTop(rows[i])}
        snapTargets={targetsFor(annotation.id)}
        {timeMode}
        onSnapChange={(snap) => (activeSnap = snap)}
        {onDuplicate}
      />
    {/each}
  {/if}

  {#if activeSnap}
    <div
      class="pointer-events-none absolute -top-25 z-40 h-50 w-px bg-lane-markup/80"
      style="left: {snapX + 6}px;"
    ></div>
    <div
      class="pointer-events-none absolute -top-25 z-40 -translate-x-1/2 rounded border border-lane-markup/60 bg-lane-markup px-1 py-0.5 font-mono text-[9px] text-background shadow-craft-sm"
      style="left: {snapX + 6}px;"
    >
      {snapLabel(activeSnap.kind)}
    </div>
  {/if}
</div>
