<script lang="ts">
  import { buildMinorTicks, buildTimeMarkers } from "./timeline-helpers";

  // Pure render: top ruler with major labels and minor ticks. Recomputes
  // markers from `duration` × `pixelsPerSecond` so the parent doesn't need
  // to thread them through.

  interface Props {
    duration: number;
    pixelsPerSecond: number;
  }

  let { duration, pixelsPerSecond }: Props = $props();

  const timeMarkers = $derived(buildTimeMarkers(duration, pixelsPerSecond));
  const minorTicks = $derived(buildMinorTicks(duration, pixelsPerSecond));
</script>

<div class="relative h-7 border-b border-border/60 bg-muted/20">
  {#each minorTicks as tick}
    <div
      class="absolute bottom-0 w-px bg-border/50"
      style="left: {tick * pixelsPerSecond}px; height: 5px;"
    ></div>
  {/each}

  {#each timeMarkers as marker}
    <div
      class="absolute top-0 flex h-full flex-col items-start"
      style="left: {marker.time * pixelsPerSecond}px;"
    >
      <div
        class="w-px bg-border"
        style="height: {marker.emphasis ? '10px' : '6px'};"
      ></div>
      <span
        class="mt-0.5 -translate-x-1/2 font-mono tabular-nums text-[10px] text-muted-foreground/80"
      >
        {marker.label}
      </span>
    </div>
  {/each}
</div>
