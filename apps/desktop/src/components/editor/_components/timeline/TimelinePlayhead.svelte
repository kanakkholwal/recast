<script lang="ts">
import { motionDuration } from "$lib/motion.svelte";
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
    <div
      class="absolute left-1/2 top-1 -translate-x-1/2 rounded border border-border bg-foreground px-1.5 py-0.5 font-mono text-[9px] tabular-nums text-background shadow-craft-sm"
    >
      {formatTimeByMode(outputTime, timeMode, fps)}
    </div>
    <div
      class="mx-auto mt-6 size-2 shrink-0 rounded-full border border-background bg-primary ring-1 ring-primary/30"
    ></div>
    <div
      class="mx-auto w-px flex-1 bg-primary/70 pointer-events-none"
      id="timeline-control"
    ></div>
  </div>
</div>
