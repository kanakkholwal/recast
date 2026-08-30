<script lang="ts" module>
export interface ToolTrimRangeProps {
	/** Total media length in seconds. 0 until metadata loads. */
	duration: number;
	start: number;
	end: number;
	onchange: (next: { start: number; end: number }) => void;
	/** Playhead position, so the range reflects where the preview actually is. */
	currentTime?: number;
	onseek?: (seconds: number) => void;
}

const MIN_SPAN = 0.1; // seconds
const KEY_STEP = 0.5;
</script>

<script lang="ts">
  import { cn } from "@recast/ui/utils";

  let {
    duration,
    start,
    end,
    onchange,
    currentTime = 0,
    onseek,
  }: ToolTrimRangeProps = $props();

  let trackEl = $state<HTMLElement | null>(null);
  let drag = $state<"start" | "end" | null>(null);

  const pct = (s: number) => (duration > 0 ? Math.min(100, Math.max(0, (s / duration) * 100)) : 0);

  const fmt = (s: number) => {
    const m = Math.floor(s / 60);
    const rest = s - m * 60;
    return `${m}:${rest.toFixed(1).padStart(4, "0")}`;
  };

  function secondsAt(clientX: number): number {
    if (!trackEl || duration <= 0) return 0;
    const rect = trackEl.getBoundingClientRect();
    const ratio = (clientX - rect.left) / rect.width;
    return Math.min(duration, Math.max(0, ratio * duration));
  }

  function grab(which: "start" | "end", e: PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = which;
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const at = secondsAt(e.clientX);
    // The handles cannot cross: each is clamped against the other.
    if (drag === "start") onchange({ start: Math.min(at, end - MIN_SPAN), end });
    else onchange({ start, end: Math.max(at, start + MIN_SPAN) });
  }

  const release = () => (drag = null);

  function key(which: "start" | "end", e: KeyboardEvent) {
    const delta =
      e.key === "ArrowLeft" ? -KEY_STEP : e.key === "ArrowRight" ? KEY_STEP : null;
    if (delta === null) return;
    e.preventDefault();
    if (which === "start") {
      onchange({ start: Math.max(0, Math.min(start + delta, end - MIN_SPAN)), end });
    } else {
      onchange({ start, end: Math.min(duration, Math.max(end + delta, start + MIN_SPAN)) });
    }
  }

  // Clicking the track scrubs the preview rather than moving a handle, so the two gestures never fight.
  function scrub(e: PointerEvent) {
    if (drag || !onseek) return;
    onseek(secondsAt(e.clientX));
  }
</script>

<div class="flex flex-col gap-2">
  <div class="text-muted-foreground flex items-center justify-between font-mono text-[11px] tabular-nums">
    <span>In {fmt(start)}</span>
    <span class="text-foreground font-medium">{fmt(Math.max(0, end - start))} selected</span>
    <span>Out {fmt(end)}</span>
  </div>

  <!-- The track scrubs; the two handles trim. -->
  <div
    bind:this={trackEl}
    class="bg-muted relative h-10 w-full cursor-pointer rounded-lg select-none"
    onpointerdown={scrub}
    onpointermove={move}
    onpointerup={release}
    onpointercancel={release}
    role="presentation"
  >
    <!-- Trimmed-away regions are dimmed, so what survives is what is bright. -->
    <div
      class="bg-background/60 absolute inset-y-0 left-0 rounded-l-lg"
      style:width={`${pct(start)}%`}
    ></div>
    <div
      class="bg-background/60 absolute inset-y-0 right-0 rounded-r-lg"
      style:width={`${100 - pct(end)}%`}
    ></div>

    <div
      class="bg-primary/20 border-primary/40 absolute inset-y-0 border-y"
      style:left={`${pct(start)}%`}
      style:width={`${Math.max(0, pct(end) - pct(start))}%`}
    ></div>

    <!-- Playhead -->
    <div
      class="bg-foreground pointer-events-none absolute inset-y-1 w-px"
      style:left={`${pct(currentTime)}%`}
    ></div>

    <button
      type="button"
      class={cn(
        "bg-primary focus-visible:ring-primary/50 absolute inset-y-0 -ml-1.5 w-3 cursor-ew-resize rounded-md outline-none focus-visible:ring-2",
        drag === "start" && "ring-primary/50 ring-2",
      )}
      style:left={`${pct(start)}%`}
      role="slider"
      aria-label="Trim start"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={start}
      aria-valuetext={`${start.toFixed(1)} seconds`}
      onpointerdown={(e) => grab("start", e)}
      onpointermove={move}
      onpointerup={release}
      onpointercancel={release}
      onkeydown={(e) => key("start", e)}
    ></button>

    <button
      type="button"
      class={cn(
        "bg-primary focus-visible:ring-primary/50 absolute inset-y-0 -ml-1.5 w-3 cursor-ew-resize rounded-md outline-none focus-visible:ring-2",
        drag === "end" && "ring-primary/50 ring-2",
      )}
      style:left={`${pct(end)}%`}
      role="slider"
      aria-label="Trim end"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={end}
      aria-valuetext={`${end.toFixed(1)} seconds`}
      onpointerdown={(e) => grab("end", e)}
      onpointermove={move}
      onpointerup={release}
      onpointercancel={release}
      onkeydown={(e) => key("end", e)}
    ></button>
  </div>
</div>
