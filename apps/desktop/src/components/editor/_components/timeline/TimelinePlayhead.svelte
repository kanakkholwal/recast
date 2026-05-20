<script lang="ts">
  import { formatTimeByMode, type TimeMode } from "./timeline-helpers";

  // Vertical playhead column with timecode pill on top and a thin guide line
  // running the height of the timeline. While the user actively drags, the
  // [left] transition is suppressed so the head pins under the cursor.

  interface Props {
    currentTime: number;
    fps: number;
    pixelsPerSecond: number;
    isDragging: boolean;
    timeMode: TimeMode;
  }

  let {
    currentTime,
    fps,
    pixelsPerSecond,
    isDragging,
    timeMode,
  }: Props = $props();

  const playheadLeft = $derived(currentTime * pixelsPerSecond);
</script>

<div
  class="absolute top-0 z-30 transition-[left] ease-out"
  style="left: {playheadLeft}px; transition-duration: {isDragging
    ? '0ms'
    : '90ms'};"
>
  <div class="relative -translate-x-1/2">
    <div
      class="absolute left-1/2 top-1 -translate-x-1/2 rounded border border-border bg-foreground px-1.5 py-0.5 font-mono text-[9px] tabular-nums text-background"
    >
      {formatTimeByMode(currentTime, timeMode, fps)}
    </div>
    <div
      class="mx-auto mt-6 size-2 rounded-full border border-background bg-primary"
    ></div>
    <div
      class="mx-auto h-57 w-px bg-primary/60 pointer-events-none"
      id="timeline-control"
    ></div>
  </div>
</div>
