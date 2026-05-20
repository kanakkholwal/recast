<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { Eye, EyeOff } from "@lucide/svelte";
  import type { TimeMode } from "./timeline-helpers";
  import { buildSnapTargets, type SnapTarget } from "./timeline-snap";
  import ZoomLayerCard from "./ZoomLayerCard.svelte";

  // Lane that hosts zoom-region cards. The lane builds the snap target list
  // (playhead + in/out + duration + neighbours) and renders a vertical
  // guide whenever any card reports an active snap during drag/resize.

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

  // Lifted from each card so the lane can paint a single guide line at the
  // active target. Last writer wins — only one card drags at a time.
  let activeSnap = $state<SnapTarget | null>(null);

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
</script>

<div
  class="relative mt-1.5 min-h-9 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-opacity"
  class:opacity-50={!store.focusEnabled}
>
  <!-- Lane label + per-lane visibility toggle. position:sticky pins it to
       the visible left edge of the horizontally-scrolling timeline so it
       never gets scrolled away or covered by a card sitting at t≈0. -->
  <div
    class="pointer-events-none sticky left-1.5 top-1 z-50 inline-flex w-fit items-center gap-1"
  >
    <span
      class="inline-flex items-center rounded-sm bg-primary/15 px-1.5 py-px font-mono text-[8px] font-bold uppercase tracking-wider text-primary backdrop-blur-sm"
    >
      Focus
    </span>
    <button
      type="button"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        store.focusEnabled = !store.focusEnabled;
      }}
      title={store.focusEnabled
        ? "Disable focus (zoom regions stay; preview & export ignore them)"
        : "Enable focus"}
      aria-label={store.focusEnabled ? "Disable focus" : "Enable focus"}
      class="pointer-events-auto flex size-4 items-center justify-center rounded text-muted-foreground hover:bg-muted/60 hover:text-foreground"
    >
      {#if store.focusEnabled}
        <Eye class="size-2.5" />
      {:else}
        <EyeOff class="size-2.5" />
      {/if}
    </button>
  </div>
  {#if store.zoomRegions.length === 0}
    <div
      class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Add a focus region to punch in during playback
    </div>
  {:else}
    {#each store.zoomRegions as region, index (region.id)}
      <ZoomLayerCard
        {store}
        {region}
        {index}
        {pixelsPerSecond}
        {fps}
        {duration}
        snapTargets={targetsFor(region.id)}
        {timeMode}
        onSnapChange={(snap) => (activeSnap = snap)}
        {onCopy}
        {onDuplicate}
      />
    {/each}
  {/if}

  {#if activeSnap}
    <!-- Snap guide. Anchored to the lane container, but drawn full-height
         using a tall element with negative offsets so it visually crosses
         the clip bar above too — the same affordance Premiere/Final Cut
         use to confirm a snap. -->
    <div
      class="pointer-events-none absolute -top-14 z-40 h-42.5 w-px bg-primary/80"
      style="left: {activeSnap.time * pixelsPerSecond + 6}px;"
    ></div>
    <div
      class="pointer-events-none absolute -top-14 z-40 -translate-x-1/2 rounded border border-primary/60 bg-primary px-1 py-0.5 font-mono text-[9px] text-primary-foreground shadow-craft-sm"
      style="left: {activeSnap.time * pixelsPerSecond + 6}px;"
    >
      {snapLabel(activeSnap.kind)}
    </div>
  {/if}
</div>

<script lang="ts" module>
  import type { SnapKind } from "./timeline-snap";

  // User-facing label for the snap badge. Kept as a top-level helper so
  // each card render doesn't allocate a fresh closure.
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
