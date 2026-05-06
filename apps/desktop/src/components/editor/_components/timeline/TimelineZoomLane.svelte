<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { X } from "@lucide/svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import { formatTime, zoomSparklinePath } from "./timeline-helpers";

  // Bottom lane that hosts zoom-region cards. Uses absolute positioning
  // overlaid on a min-height container so cards can stack visually when they
  // overlap in time. The cards themselves anchor relative to the *outer*
  // timeline track because they read absolute time × pixelsPerSecond.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
  }

  let { store, pixelsPerSecond }: Props = $props();
</script>

<div
  class="mt-1.5 min-h-9 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5"
>
  {#if store.zoomRegions.length === 0}
    <div
      class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Add a focus region to punch in during playback
    </div>
  {:else}
    {#each store.zoomRegions as region, index (region.id)}
      {@const isSelected = region.id === store.selectedZoomRegionId}
      <button
        type="button"
        onclick={(e) => {
          e.stopPropagation();
          store.selectedZoomRegionId = region.id;
        }}
        in:fly={{ y: 10, duration: 180, easing: cubicOut }}
        out:fade={{ duration: 140 }}
        aria-pressed={isSelected}
        class="z-50 absolute overflow-hidden rounded border bg-muted text-left transition-colors focus:outline-none focus:ring-1 focus:ring-ring {isSelected
          ? 'border-primary ring-1 ring-primary/40'
          : 'border-border hover:border-primary/60'}"
        style="
          left: {region.start * pixelsPerSecond}px;
          width: {Math.max((region.end - region.start) * pixelsPerSecond, 56)}px;
          top: {86 + index * 2}px;
          height: 30px;
        "
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
          class="relative flex h-full items-center justify-between gap-2 px-2"
          id={`zoom-region-${region.id}`}
          aria-label={`Focus region from ${formatTime(region.start)} to ${formatTime(region.end)}, scale ${region.scale.toFixed(1)}x. Click to select, or press Enter or Space when focused. Press the X button to remove.`}
        >
          <div class="min-w-0">
            <p class="text-[10px] font-semibold text-foreground">Focus</p>
            <p class="text-[9px] text-muted-foreground">
              {region.scale.toFixed(1)}x · {formatTime(region.start)}
            </p>
          </div>
          <span
            role="button"
            id={`remove-zoom-region-${region.id}`}
            tabindex="0"
            onclick={(event) => {
              event.stopPropagation();
              store.removeZoomRegion(region.id);
            }}
            onpointerdown={(event) => {
              event.stopPropagation();
              store.removeZoomRegion(region.id);
            }}
            onkeydown={(event) => {
              event.preventDefault();
              event.stopPropagation();
              if (event.key === "Enter" || event.key === " ") {
                store.removeZoomRegion(region.id);
              }
            }}
            class="flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded border border-border bg-background/70 text-muted-foreground transition-colors hover:border-destructive hover:text-destructive pointer-events-auto"
            aria-label="Remove focus region"
          >
            <X size={9} strokeWidth={2.5} />
          </span>
        </div>
      </button>
    {/each}
  {/if}
</div>
