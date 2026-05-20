<script lang="ts">
  import type { Annotation, EditorStore } from "$lib/stores/editor-store.svelte";
  import { Eye, EyeOff } from "@lucide/svelte";
  import type { TimeMode } from "./timeline-helpers";
  import { buildSnapTargets, type SnapTarget } from "./timeline-snap";
  import AnnotationLayerCard from "./AnnotationLayerCard.svelte";

  // Annotation lane — sister of TimelineZoomLane. Same lifted-state pattern
  // for the snap guide so visual feedback during a drag is consistent
  // between lanes.

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
</script>

<div
  class="relative mt-1.5 min-h-9 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-opacity"
  class:opacity-50={store.annotationsGloballyHidden}
>
  <div
    class="pointer-events-none sticky left-1.5 top-1 z-50 inline-flex w-fit items-center gap-1"
  >
    <span
      class="inline-flex items-center rounded-sm bg-amber-500/20 px-1.5 py-px font-mono text-[8px] font-bold uppercase tracking-wider text-amber-700 backdrop-blur-sm dark:text-amber-400"
    >
      Notes
    </span>
    <button
      type="button"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        store.annotationsGloballyHidden = !store.annotationsGloballyHidden;
      }}
      title={store.annotationsGloballyHidden
        ? "Enable notes"
        : "Disable notes (annotations stay; preview & export ignore them)"}
      aria-label={store.annotationsGloballyHidden
        ? "Enable notes"
        : "Disable notes"}
      class="pointer-events-auto flex size-4 items-center justify-center rounded text-muted-foreground hover:bg-muted/60 hover:text-foreground"
    >
      {#if !store.annotationsGloballyHidden}
        <Eye class="size-2.5" />
      {:else}
        <EyeOff class="size-2.5" />
      {/if}
    </button>
  </div>
  {#if store.annotations.length === 0}
    <div
      class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Annotations you draw on the preview appear here as draggable layers
    </div>
  {:else}
    {#each store.annotations as annotation, index (annotation.id)}
      <AnnotationLayerCard
        {store}
        {annotation}
        {index}
        {pixelsPerSecond}
        {fps}
        {duration}
        snapTargets={targetsFor(annotation.id)}
        {timeMode}
        onSnapChange={(snap) => (activeSnap = snap)}
        {onDuplicate}
      />
    {/each}
  {/if}

  {#if activeSnap}
    <div
      class="pointer-events-none absolute -top-25 z-40 h-50 w-px bg-amber-500/80"
      style="left: {activeSnap.time * pixelsPerSecond + 6}px;"
    ></div>
    <div
      class="pointer-events-none absolute -top-25 z-40 -translate-x-1/2 rounded border border-amber-500/60 bg-amber-500 px-1 py-0.5 font-mono text-[9px] text-amber-50 shadow-craft-sm"
      style="left: {activeSnap.time * pixelsPerSecond + 6}px;"
    >
      {snapLabel(activeSnap.kind)}
    </div>
  {/if}
</div>

<script lang="ts" module>
  import type { SnapKind } from "./timeline-snap";

  function snapLabel(kind: SnapKind): string {
    switch (kind) {
      case "playhead":
        return "Playhead";
      case "in-point":
        return "In";
      case "out-point":
        return "Out";
      case "origin":
        return "Start";
      case "duration":
        return "End";
      case "region-start":
        return "Region start";
      case "region-end":
        return "Region end";
      case "annotation-start":
        return "Annotation start";
      case "annotation-end":
        return "Annotation end";
      case "frame":
        return "Frame";
    }
  }
</script>
