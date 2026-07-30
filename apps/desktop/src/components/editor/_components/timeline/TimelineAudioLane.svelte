<script lang="ts">
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { originalToOutput } from "$lib/timeline/time-map";
import { CLIP_LABEL, clipSurface } from "./timeline-clip.styles";
import { buildWaveformPath } from "./timeline-helpers";
import { AUDIO_LANE_HEIGHT_PX, LANE_PADDING_PX, ROW_HEIGHT_PX } from "./timeline-stack";

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

// Shared with the track rail, which sizes its label row from the same constant.
const LANE_H = AUDIO_LANE_HEIGHT_PX;
/** The block inside the lane, the same height as a block in any other lane. */
const BLOCK_H = ROW_HEIGHT_PX;

// Same output-axis mapping as every other lane: a removed range collapses onto
// its seam, so the envelope stays aligned with the frames above it.
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
const axisWidth = $derived(Math.max(0, xOf(duration)));

const waveformPath = $derived(
	buildWaveformPath({
		waveform: store.waveform,
		duration,
		xOf,
		height: BLOCK_H,
		amp: BLOCK_H / 2 - 3,
		range: { start: store.inPoint, end: store.outPoint },
	}),
);

const hasAudio = $derived(!!store.audioPath || !!store.microphonePath);
const surface = clipSurface("audio");

// Names what the envelope actually is. The waveform is a MIX of whichever
// sources were captured, and nothing else in the editor says which those were.
const sourceLabel = $derived.by(() => {
	const system = !!store.audioPath;
	const mic = !!store.microphonePath;
	if (system && mic) return "System + Mic";
	if (mic) return "Microphone";
	return "System audio";
});
</script>

<div
  class="relative mt-1.5 overflow-hidden rounded-md bg-muted/20"
  style="height: {LANE_H}px;"
>
  {#if waveformPath && axisWidth > 0}
    <!-- The track reads as ONE audio clip: a solid body in the lane's fill with
         the envelope drawn inside it, the way an NLE draws an audio item. It
         used to be a bare path on the lane background, which made the recording's
         own audio the only lane with no object in it. -->
    <div
      class="absolute left-0 overflow-hidden rounded-[4px] {surface.fill}"
      style="top: {LANE_PADDING_PX}px; width: {axisWidth}px; height: {BLOCK_H}px;"
    >
      <svg
        class="pointer-events-none absolute inset-0"
        style="width: {axisWidth}px; height: {BLOCK_H}px;"
        viewBox="0 0 {axisWidth} {BLOCK_H}"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <!-- Centre line, so silence reads as silence rather than as a gap.
             Halves BLOCK_H, not the lane height: the viewBox is the block. -->
        <line
          x1="0"
          y1={BLOCK_H / 2}
          x2={axisWidth}
          y2={BLOCK_H / 2}
          class="stroke-lane-on/25"
          stroke-width="1"
          vector-effect="non-scaling-stroke"
        />
        <path d={waveformPath} class={surface.wave} />
      </svg>

      <!-- Named like any other clip. The scrim keeps it legible where a loud
           passage pushes the envelope up behind it. -->
      <div
        class="pointer-events-none absolute inset-x-0 top-0 flex h-4 items-center bg-linear-to-b from-black/25 to-transparent px-1.5"
      >
        <span class={CLIP_LABEL}>{sourceLabel}</span>
      </div>
    </div>
  {:else}
    <div
      class="flex h-full items-center justify-center text-[11px] text-muted-foreground"
    >
      {hasAudio ? "Reading the audio track…" : "This recording has no audio"}
    </div>
  {/if}
</div>
