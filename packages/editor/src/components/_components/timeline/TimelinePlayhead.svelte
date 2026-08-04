<script lang="ts">
import { motionDuration } from "../../../lib/motion.svelte";
import { formatTimeByMode, type TimeMode } from "./timeline-helpers";

// Positioned with `transform: translate3d`, not `left`: `left` relays out the
// whole track every frame of playback, and its transition made the head lag the
// picture. transform is composited and cheap.
//
// The tween exists only to smooth a discrete jump (click-to-seek, frame step).
// During playback the store already publishes a fresh position every frame, so
// any tween there is pure lag: suppressed, like it is while dragging. Reduced
// motion drops it everywhere.

interface Props {
	/** Seconds on the OUTPUT (post-cut) axis: the same axis as the ruler under
	 *  the head and the transport readout above it. Never original time. */
	outputTime: number;
	/** px on the output axis. */
	leftPx: number;
	fps: number;
	isDragging: boolean;
	isPlaying: boolean;
	timeMode: TimeMode;
}

let { outputTime, leftPx, fps, isDragging, isPlaying, timeMode }: Props = $props();

const tweenMs = $derived(isDragging || isPlaying ? 0 : motionDuration(90));
</script>

<!-- Spans the full track height via inset-y-0 so it tracks however many lanes are
     shown; the guide line flexes to fill below the head. -->
<div
  class="absolute inset-y-0 left-0 z-30 transition-transform ease-out will-change-transform"
  style="transform: translate3d({leftPx}px, 0, 0); transition-duration: {tweenMs}ms;"
>
  <div class="relative flex h-full flex-col -translate-x-1/2">
    <!-- Timecode only while scrubbing. It used to sit above the head at all
         times, which put a permanent chip over the first lane for a reading the
         transport already shows. -->
    {#if isDragging}
      <div
        class="absolute left-1/2 top-0 -translate-x-1/2 rounded bg-foreground px-1.5 py-0.5 font-mono text-[9px] tabular-nums text-background shadow-craft-sm"
      >
        {formatTimeByMode(outputTime, timeMode, fps)}
      </div>
    {/if}
    <!-- Marker sits in the ruler band and tapers into the line, so the head
         reads as the top of the playhead rather than a dot floating above it. -->
    <div
      class="mx-auto mt-1.5 h-3.5 w-2.5 shrink-0 rounded-[2px] rounded-b-[3px] bg-primary shadow-craft-sm"
    ></div>
    <div
      class="mx-auto w-px flex-1 bg-primary pointer-events-none"
      id="timeline-control"
    ></div>
  </div>
</div>
