<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { originalToOutput } from "$lib/timeline/time-map";
  import { buildWaveformPath } from "./timeline-helpers";

  // The audio track gets its own lane, like every real NLE.
  //
  // It used to be an either-or radio with the clip thumbnails, then a 16px strip
  // squeezed along the bottom of the clip bar. Neither works: cutting dead air is
  // the most common task in a screen recorder and it needs the waveform AND the
  // frames legible at the same time. A dedicated lane gives the envelope real
  // height and leaves the clip bar alone.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    duration: number;
  }

  let { store, pixelsPerSecond, duration }: Props = $props();

  // Full lane height, so quiet passages are still readable.
  const LANE_H = 36;

  // Same output-axis mapping as every other lane: a removed range collapses onto
  // its seam, so the envelope stays aligned with the frames above it.
  const xOf = (t: number) =>
    originalToOutput(store.renderMap, t) * pixelsPerSecond;
  const axisWidth = $derived(Math.max(0, xOf(duration)));

  const waveformPath = $derived(
    buildWaveformPath({
      waveform: store.waveform,
      duration,
      xOf,
      height: LANE_H,
      amp: LANE_H / 2 - 2,
      range: { start: store.inPoint, end: store.outPoint },
    }),
  );

  const hasAudio = $derived(!!store.audioPath || !!store.microphonePath);
</script>

<div
  class="relative mt-1.5 overflow-hidden rounded-md border border-border/60 bg-background/40"
  style="height: {LANE_H}px;"
>
  {#if waveformPath && axisWidth > 0}
    <svg
      class="pointer-events-none absolute inset-y-0 left-0"
      style="width: {axisWidth}px; height: {LANE_H}px;"
      viewBox="0 0 {axisWidth} {LANE_H}"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <!-- Centre line, so silence reads as silence rather than as a gap. -->
      <line
        x1="0"
        y1={LANE_H / 2}
        x2={axisWidth}
        y2={LANE_H / 2}
        class="stroke-border"
        stroke-width="1"
        vector-effect="non-scaling-stroke"
      />
      <path d={waveformPath} class="fill-lane-audio/60" />
    </svg>
  {:else}
    <div
      class="flex h-full items-center justify-center text-[10px] text-muted-foreground"
    >
      {hasAudio ? "Reading the audio track…" : "This recording has no audio"}
    </div>
  {/if}
</div>
