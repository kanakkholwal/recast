<script lang="ts">
import { buildMinorTicks, buildTimeMarkers, type TimeMode } from "./timeline-helpers";

interface Props {
	/** OUTPUT (post-cut) seconds, matching the axis every lane is drawn on. */
	duration: number;
	pixelsPerSecond: number;
	/** Ticks format through the shared clock, so Frames mode reaches the ruler
	 *  instead of only the playhead. */
	timeMode: TimeMode;
	fps: number;
	/** Visible scroll window in px. Ticks are built only for this slice — a
	 *  30-min project at 0.25s spacing is 7,200 tick nodes, and they used to all
	 *  live in the layout tree at once regardless of what was on screen. */
	viewportLeftPx?: number;
	viewportWidthPx?: number;
}

let { duration, pixelsPerSecond, timeMode, fps, viewportLeftPx, viewportWidthPx }: Props = $props();

// One screen of overscan each side so a scroll doesn't reveal a bare edge
// before the next frame builds the ticks.
const window_ = $derived.by(() => {
	if (viewportLeftPx === undefined || !viewportWidthPx || pixelsPerSecond <= 0) return undefined;
	return {
		startSec: Math.max(0, (viewportLeftPx - viewportWidthPx) / pixelsPerSecond),
		endSec: (viewportLeftPx + viewportWidthPx * 2) / pixelsPerSecond,
	};
});

const timeMarkers = $derived(buildTimeMarkers(duration, pixelsPerSecond, timeMode, fps, window_));
const minorTicks = $derived(buildMinorTicks(duration, pixelsPerSecond, window_));
</script>

<div class="relative h-7 border-b border-border/60 bg-muted/20">
  {#each minorTicks as tick (tick)}
    <div
      class="absolute bottom-0 w-px bg-border/50"
      style="left: {tick * pixelsPerSecond}px; height: 5px;"
    ></div>
  {/each}

  {#each timeMarkers as marker (marker.time)}
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
