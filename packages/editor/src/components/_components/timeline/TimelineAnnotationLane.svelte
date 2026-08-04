<script lang="ts">
import type { Annotation, EditorStore } from "$lib/stores/editor-store.svelte";
import { originalToOutput } from "$lib/timeline/time-map";
import type { TimeMode } from "./timeline-helpers";
import { buildSnapTargets, snapLabel, type SnapTarget } from "./timeline-snap";
import type { LaneCardLayout } from "./timeline-stack";
import AnnotationLayerCard from "./AnnotationLayerCard.svelte";

// Sister of TimelineZoomLane; same lifted snap-guide pattern.

interface Props {
	store: EditorStore;
	pixelsPerSecond: number;
	fps: number;
	duration: number;
	timeMode: TimeMode;
	/** Card placement + lane height, computed by the timeline so the track rail
	 *  and this lane can never disagree on how tall the lane is. */
	layout: LaneCardLayout;
	onDuplicate: (annotation: Annotation) => void;
}

let { store, pixelsPerSecond, fps, duration, timeMode, layout, onDuplicate }: Props = $props();

let activeSnap = $state<SnapTarget | null>(null);
const snapX = $derived(
	activeSnap ? originalToOutput(store.renderMap, activeSnap.time) * pixelsPerSecond : 0,
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
</script>

<div
  class="relative mt-1.5 rounded-md bg-muted/20 px-1.5 py-1.5 transition-[opacity,height]"
  class:opacity-50={store.annotationsGloballyHidden}
  style="height: {layout.height}px;"
>
  {#if store.annotations.length === 0}
    <div
      class="flex h-8 items-center justify-center text-[11px] text-muted-foreground"
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
        left={layout.cards[i].left}
        width={layout.cards[i].width}
        top={layout.cards[i].top}
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
