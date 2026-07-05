<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { type TimelineCut } from "$lib/timeline/cuts";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { X } from "@lucide/svelte";
  import { buildWaveformPath } from "./timeline-helpers";
  import { clampCutMove, clampCutResize } from "./timeline-cutlane.logic";

  // Hosts cut bands. Drag empty lane space to carve a cut; drag a band's edges or body to adjust it.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    duration: number;
  }

  let { store, pixelsPerSecond, duration }: Props = $props();

  // Cuts shorter than this are dropped — a sub-100ms removal reads as a glitch.
  const MIN_CUT = 0.1;

  let laneEl = $state<HTMLDivElement | null>(null);

  // Output axis via the shared display map. An applied cut collapses to zero
  // width (rendered as a seam); an unapplied cut (lane off → not in the map's
  // cuts) keeps its width as an editable band.
  const xOf = (t: number) => originalToOutput(store.timeMap, t) * pixelsPerSecond;
  const axisWidth = $derived(xOf(duration));

  type DragMode = "create" | "move" | "resize-l" | "resize-r";
  interface DragState {
    mode: DragMode;
    pointerId: number;
    /** null until a create-drag has actually spawned a cut. */
    id: string | null;
    anchorTime: number;
    originStart: number;
    originEnd: number;
  }
  let drag = $state<DragState | null>(null);

  function timeAt(clientX: number): number {
    if (!laneEl) return 0;
    const x = clientX - laneEl.getBoundingClientRect().left;
    // Pointer is in OUTPUT pixels → output seconds → original time.
    return Math.min(duration, Math.max(0, outputToOriginal(store.timeMap, x / pixelsPerSecond)));
  }

  function onLaneDown(e: PointerEvent) {
    // Only the bare lane background starts a create-drag — bands and their
    // handles stop propagation in their own handlers.
    if (e.target !== laneEl || duration <= 0) return;
    // Stop the timeline's scrub handler from also claiming this drag.
    e.preventDefault();
    e.stopPropagation();
    const t = timeAt(e.clientX);
    drag = {
      mode: "create",
      pointerId: e.pointerId,
      id: null,
      anchorTime: t,
      originStart: t,
      originEnd: t,
    };
    laneEl?.setPointerCapture(e.pointerId);
  }

  function onBandDown(e: PointerEvent, cut: TimelineCut, mode: DragMode) {
    e.preventDefault();
    e.stopPropagation();
    if (!laneEl) return;
    // A drag is one discrete action → one undo entry.
    store.pushUndoState();
    drag = {
      mode,
      pointerId: e.pointerId,
      id: cut.id,
      anchorTime: timeAt(e.clientX),
      originStart: cut.start,
      originEnd: cut.end,
    };
    laneEl.setPointerCapture(e.pointerId);
  }

  function onMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const t = timeAt(e.clientX);

    if (drag.mode === "create") {
      const lo = Math.min(drag.anchorTime, t);
      const hi = Math.max(drag.anchorTime, t);
      if (drag.id === null) {
        if (hi - lo < MIN_CUT) return; // not a deliberate drag yet
        drag.id = store.addCut(lo, hi, "manual");
      } else {
        store.updateCut(drag.id, lo, hi);
      }
      return;
    }

    if (!drag.id) return;
    const delta = t - drag.anchorTime;
    const next =
      drag.mode === "move"
        ? clampCutMove({
            originStart: drag.originStart,
            originEnd: drag.originEnd,
            delta,
            duration,
          })
        : clampCutResize({
            edge: drag.mode === "resize-l" ? "l" : "r",
            originStart: drag.originStart,
            originEnd: drag.originEnd,
            delta,
            duration,
            minCut: MIN_CUT,
          });
    store.updateCut(drag.id, next.start, next.end);
  }

  function onUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    // Fold any cut a drag pushed into a neighbour into one clean band.
    if (drag.id) store.mergeCuts();
    laneEl?.releasePointerCapture(e.pointerId);
    drag = null;
  }

  function remove(e: Event, id: string) {
    e.stopPropagation();
    store.removeCut(id);
  }

  // Peak envelope behind the bands, spanning the whole axis (buckets inside an
  // applied cut collapse onto the seam via xOf).
  const waveformPath = $derived(
    buildWaveformPath({
      waveform: store.waveform,
      duration,
      xOf,
      height: 100,
      amp: 46,
    }),
  );
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointerdown={onLaneDown}
  onpointermove={onMove}
  onpointerup={onUp}
  onpointercancel={onUp}
  class="relative mt-1.5 min-h-9 cursor-crosshair rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-opacity"
  class:opacity-50={!store.cutsEnabled}
>
  {#if waveformPath}
    <svg
      class="pointer-events-none absolute left-0 top-1.5 bottom-1.5"
      style="width: {axisWidth}px;"
      viewBox="0 0 {axisWidth} 100"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <path d={waveformPath} class="fill-foreground/20" />
    </svg>
  {/if}

  {#if store.cuts.length === 0}
    <div
      class="pointer-events-none flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Drag across this lane to remove a section
    </div>
  {/if}

  {#each store.cuts as cut (cut.id)}
    {@const cutLeft = xOf(cut.start)}
    {@const cutW = xOf(cut.end) - cutLeft}
    {#if cutW < 2}
      <!-- Applied cut collapsed to a seam (click to restore). Move/resize need
           width, so they only work on the unapplied band below. -->
      <button
        type="button"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={(e) => remove(e, cut.id)}
        title="Removed {(cut.end - cut.start).toFixed(2)}s. Click to restore."
        aria-label="Restore this section"
        class="group/seam absolute top-1.5 bottom-1.5 z-6 w-3 -translate-x-1/2 cursor-pointer"
        style="left: {cutLeft}px;"
      >
        <div
          class="mx-auto h-full w-0.5 bg-destructive/70 transition-all group-hover/seam:w-1 group-hover/seam:bg-destructive"
        ></div>
        <span
          class="pointer-events-none absolute bottom-full left-1/2 mb-1 hidden -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm group-hover/seam:block"
        >
          −{(cut.end - cut.start).toFixed(2)}s · restore
        </span>
      </button>
    {:else}
      {@const w = Math.max(8, cutW)}
      <div
        role="presentation"
        onpointerdown={(e) => onBandDown(e, cut, "move")}
        title="Removed section · {(cut.end - cut.start).toFixed(2)}s"
        class="group/cut absolute top-1.5 bottom-1.5 cursor-grab overflow-hidden rounded-sm border border-destructive/50 bg-destructive/20 active:cursor-grabbing"
        style="left: {cutLeft}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--destructive) 22%, transparent) 5px, color-mix(in srgb, var(--destructive) 22%, transparent) 10px);"
      >
        <!-- Edge resize handles -->
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-l")}
          class="absolute inset-y-0 left-0 w-1.5 cursor-ew-resize bg-destructive/60 opacity-0 transition-opacity group-hover/cut:opacity-100"
        ></div>
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-r")}
          class="absolute inset-y-0 right-0 w-1.5 cursor-ew-resize bg-destructive/60 opacity-0 transition-opacity group-hover/cut:opacity-100"
        ></div>

        {#if w > 44}
          <span
            class="pointer-events-none absolute inset-0 flex items-center justify-center font-mono text-[8px] font-bold text-destructive"
          >
            −{(cut.end - cut.start).toFixed(1)}s
          </span>
        {/if}

        <button
          type="button"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={(e) => remove(e, cut.id)}
          aria-label="Restore this section"
          title="Restore this section"
          class="absolute right-0.5 top-0.5 flex size-3.5 items-center justify-center rounded bg-destructive text-destructive-foreground opacity-0 transition-opacity hover:scale-110 group-hover/cut:opacity-100"
        >
          <X class="size-2.5" />
        </button>
      </div>
    {/if}
  {/each}
</div>
