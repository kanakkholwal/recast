<script lang="ts">
  import {
    smoothCursorPath,
    smoothingStrengthToSigmaMs,
    type CursorSampleLike,
  } from "$lib/cursor/smoothing";

  interface Props {
    samples: CursorSampleLike[];
    /** Video width in pixels (used to normalise the trajectory). */
    videoWidth: number;
    videoHeight: number;
    smoothing: number;
    snapToClicks: boolean;
    snapWindowMs: number;
    /** Max pixels of panel width to render into. Height is derived from aspect. */
    width?: number;
    /**
     * If set, sample the trajectory to this many points along the full path
     * (keeps SVG cheap on long recordings). Default 400.
     */
    maxPoints?: number;
  }

  let {
    samples,
    videoWidth,
    videoHeight,
    smoothing,
    snapToClicks,
    snapWindowMs,
    width = 220,
    maxPoints = 400,
  }: Props = $props();

  const aspect = $derived(videoHeight > 0 ? videoHeight / videoWidth : 9 / 16);
  const height = $derived(Math.round(width * aspect));

  const smoothed = $derived.by(() => {
    return smoothCursorPath(samples, {
      sigmaMs: smoothingStrengthToSigmaMs(smoothing),
      snapToClicks,
      snapWindowMs,
    });
  });

  function decimate<T>(arr: T[], target: number): T[] {
    if (arr.length <= target) return arr;
    const out: T[] = new Array(target);
    const step = arr.length / target;
    for (let i = 0; i < target; i++) {
      out[i] = arr[Math.floor(i * step)];
    }
    return out;
  }

  // Build an SVG path from sample points normalised to viewBox [0..1, 0..1].
  function pathFrom(points: { x: number; y: number }[]): string {
    if (points.length === 0 || videoWidth <= 0 || videoHeight <= 0) return "";
    const pts = decimate(points, maxPoints);
    const xs = (p: { x: number }) => p.x / videoWidth;
    const ys = (p: { y: number }) => p.y / videoHeight;
    return pts.map((p, i) => `${i === 0 ? "M" : "L"} ${xs(p).toFixed(4)} ${ys(p).toFixed(4)}`).join(" ");
  }

  const rawPath = $derived(pathFrom(samples));
  const smoothedPath = $derived(pathFrom(smoothed.samples));

  const clickMarks = $derived.by(() => {
    if (videoWidth <= 0 || videoHeight <= 0) return [] as { x: number; y: number }[];
    return smoothed.clickAnchors.map((c) => ({
      x: c.x / videoWidth,
      y: c.y / videoHeight,
    }));
  });

  const isEmpty = $derived(samples.length === 0 || videoWidth <= 0 || videoHeight <= 0);
</script>

<div
  class="relative w-full overflow-hidden rounded-md border border-border bg-card/50"
  style:height="{height}px"
>
  {#if isEmpty}
    <div class="absolute inset-0 flex items-center justify-center text-[10px] text-muted-foreground">
      No cursor data in this clip
    </div>
  {:else}
    <svg
      viewBox="0 0 1 1"
      preserveAspectRatio="none"
      role="img"
      aria-label="Cursor trajectory — raw vs. smoothed"
      class="block h-full w-full"
    >
      <!-- Raw path: dashed, low-contrast so it reads as "before". -->
      <path
        d={rawPath}
        fill="none"
        stroke="currentColor"
        stroke-width="0.004"
        stroke-dasharray="0.008 0.006"
        class="text-muted-foreground/70"
        vector-effect="non-scaling-stroke"
      />
      <!-- Smoothed path: solid primary so it reads as "after". -->
      <path
        d={smoothedPath}
        fill="none"
        stroke="currentColor"
        stroke-width="0.006"
        class="text-primary"
        vector-effect="non-scaling-stroke"
      />
      <!-- Click markers — anchored exactly to the press position. -->
      {#each clickMarks as mark, i (i)}
        <circle
          cx={mark.x}
          cy={mark.y}
          r="0.012"
          fill="currentColor"
          class="text-primary"
        />
        <circle
          cx={mark.x}
          cy={mark.y}
          r="0.020"
          fill="none"
          stroke="currentColor"
          stroke-width="0.003"
          class="text-primary/60"
          vector-effect="non-scaling-stroke"
        />
      {/each}
    </svg>
    <!-- Legend -->
    <div
      class="pointer-events-none absolute right-1 bottom-1 flex items-center gap-2 rounded border border-border bg-background/70 px-1.5 py-0.5 text-[9px] text-muted-foreground backdrop-blur-sm"
    >
      <span class="flex items-center gap-1">
        <svg width="10" height="4" aria-hidden="true">
          <line x1="0" y1="2" x2="10" y2="2" stroke="currentColor" stroke-width="1.2" stroke-dasharray="2 1.5" />
        </svg>
        Raw
      </span>
      <span class="flex items-center gap-1 text-primary">
        <svg width="10" height="4" aria-hidden="true">
          <line x1="0" y1="2" x2="10" y2="2" stroke="currentColor" stroke-width="1.6" />
        </svg>
        Smoothed
      </span>
    </div>
  {/if}
</div>
