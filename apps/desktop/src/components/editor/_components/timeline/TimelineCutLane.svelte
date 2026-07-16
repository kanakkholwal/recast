<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { type TimelineCut } from "$lib/timeline/cuts";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { Scissors, X } from "@recast/icons";
  import { buildWaveformPath } from "./timeline-helpers";
  import { clampCutMove, clampCutResize } from "./timeline-cutlane.logic";

  // Hosts cut bands. Drag empty lane space to carve a cut; drag a band's edges or body to adjust it.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    duration: number;
    /** Draw the faint audio envelope behind the bands. Off when the dedicated
     *  Audio lane is visible right above (it already shows the same data), on
     *  when that lane is hidden so you can still cut against the audio. */
    showWaveform?: boolean;
  }

  let { store, pixelsPerSecond, duration, showWaveform = true }: Props =
    $props();

  // Cuts shorter than this are dropped. A sub-100ms removal reads as a glitch.
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
    /** The cut being adjusted. Always null while creating: see `pending`. */
    id: string | null;
    anchorTime: number;
    originStart: number;
    originEnd: number;
  }
  let drag = $state<DragState | null>(null);

  // A create-drag is PREVIEWED, then committed on release.
  //
  // It used to call addCut() as soon as the span passed MIN_CUT, which applied
  // the cut immediately: the time map collapsed the removed range, every later
  // frame slid left, and the original time under the pointer changed mid-gesture.
  // The band you were dragging shrank to a seam under your own cursor and the
  // drag came apart. Nothing touches the store now until pointerup.
  let pending = $state<{ start: number; end: number } | null>(null);

  function timeAt(clientX: number): number {
    if (!laneEl) return 0;
    const x = clientX - laneEl.getBoundingClientRect().left;
    // Pointer is in OUTPUT pixels → output seconds → original time.
    return Math.min(duration, Math.max(0, outputToOriginal(store.timeMap, x / pixelsPerSecond)));
  }

  function onLaneDown(e: PointerEvent) {
    // Only the bare lane background starts a create-drag; bands and their
    // handles stop propagation in their own handlers. Left button only: a
    // right-drag is for the context menu, not for carving a cut.
    if (e.target !== laneEl || duration <= 0 || e.button !== 0) return;
    // The razor tool owns clicks timeline-wide: let this one bubble to the
    // scroller's razor handler instead of starting a create-drag.
    if (store.timelineTool === "razor") return;
    // Bypassed track: refuse the edit rather than carve a cut that silently
    // wouldn't apply. The inline hint says why.
    if (!store.cutsEnabled) return;
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
    pending = null;
    laneEl?.setPointerCapture(e.pointerId);
  }

  function onBandDown(e: PointerEvent, cut: TimelineCut, mode: DragMode) {
    // Left button only; let a razor click carve through the band, not move it.
    if (e.button !== 0) return;
    if (store.timelineTool === "razor") return;
    // Bypassed track: no move/resize (the X to restore a cut still works).
    if (!store.cutsEnabled) return;
    e.preventDefault();
    e.stopPropagation();
    if (!laneEl) return;
    // Selecting the band makes document-level Delete restore this exact cut.
    store.selectedCutId = cut.id;
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
      // Preview only. The map stays put, so `timeAt` keeps tracking the cursor.
      pending = hi - lo >= MIN_CUT ? { start: lo, end: hi } : null;
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
    // Commit the previewed span now, as one undo entry. addCut() pushes the undo
    // state itself, so pushing here too would leave a duplicate snapshot: the
    // first Ctrl+Z removes the cut, the second re-applies the same state and
    // looks like undo is broken.
    if (drag.mode === "create" && pending) {
      const id = store.addCut(pending.start, pending.end, "manual");
      if (id) store.mergeCuts();
    } else if (drag.id) {
      // Fold any cut a drag pushed into a neighbour into one clean band.
      store.mergeCuts();
    }
    pending = null;
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
    showWaveform
      ? buildWaveformPath({
          waveform: store.waveform,
          duration,
          xOf,
          height: 100,
          amp: 46,
        })
      : "",
  );
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointerdown={onLaneDown}
  onpointermove={onMove}
  onpointerup={onUp}
  onpointercancel={onUp}
  class="relative mt-1.5 min-h-9 cursor-crosshair rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5"
>
  {#if showWaveform && waveformPath}
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

  <!-- Live preview of the span the release will remove. -->
  {#if pending}
    {@const px = xOf(pending.start)}
    {@const pw = Math.max(2, xOf(pending.end) - px)}
    <div
      class="pointer-events-none absolute top-1.5 bottom-1.5 z-10 rounded-sm border border-lane-cut/70 bg-lane-cut/25"
      style="left: {px}px; width: {pw}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-cut) 22%, transparent) 5px, color-mix(in srgb, var(--lane-cut) 22%, transparent) 10px);"
    >
      {#if pw > 44}
        <span
          class="pointer-events-none absolute inset-0 flex items-center justify-center font-mono text-[8px] font-bold text-lane-cut"
        >
          −{(pending.end - pending.start).toFixed(1)}s
        </span>
      {/if}
    </div>
  {/if}

  {#if !store.cutsEnabled}
    <!-- Bypassed: say why editing is refused rather than dimming silently. -->
    <div
      class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center gap-1.5 rounded-md bg-background/60 text-[10px] font-medium text-foreground"
    >
      <Scissors class="size-3 text-lane-cut" />
      Cuts are off. Turn on "Apply cuts" in Layers to edit.
    </div>
  {:else if store.cuts.length === 0 && !pending}
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
          class="mx-auto h-full w-0.5 bg-lane-cut/70 transition-all group-hover/seam:w-1 group-hover/seam:bg-lane-cut"
        ></div>
        <span
          class="pointer-events-none absolute bottom-full left-1/2 mb-1 hidden -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm group-hover/seam:block"
        >
          −{(cut.end - cut.start).toFixed(2)}s · restore
        </span>
      </button>
    {:else}
      {@const w = Math.max(8, cutW)}
      {@const isSel = store.selectedCutId === cut.id}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        role="button"
        tabindex="0"
        data-selectable
        aria-pressed={isSel}
        aria-label={`Removed section, ${(cut.end - cut.start).toFixed(2)} seconds. Drag to move; press Delete to restore.`}
        onpointerdown={(e) => onBandDown(e, cut, "move")}
        onfocus={() => (store.selectedCutId = cut.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            store.selectedCutId = cut.id;
          }
        }}
        title="Removed section · {(cut.end - cut.start).toFixed(2)}s"
        class="group/cut absolute top-1.5 bottom-1.5 cursor-grab overflow-hidden rounded-sm border bg-lane-cut/20 transition-shadow active:cursor-grabbing focus-visible:outline-none {isSel
          ? 'border-lane-cut ring-2 ring-lane-cut/50'
          : 'border-lane-cut/50'}"
        style="left: {cutLeft}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-cut) 22%, transparent) 5px, color-mix(in srgb, var(--lane-cut) 22%, transparent) 10px);"
      >
        <!-- Edge resize handles -->
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-l")}
          class="absolute inset-y-0 left-0 w-1.5 cursor-ew-resize bg-lane-cut/60 opacity-0 transition-opacity group-hover/cut:opacity-100"
        ></div>
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-r")}
          class="absolute inset-y-0 right-0 w-1.5 cursor-ew-resize bg-lane-cut/60 opacity-0 transition-opacity group-hover/cut:opacity-100"
        ></div>

        {#if w > 44}
          <span
            class="pointer-events-none absolute inset-0 flex items-center justify-center font-mono text-[8px] font-bold text-lane-cut"
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
          class="absolute right-0.5 top-0.5 flex size-3.5 items-center justify-center rounded bg-lane-cut text-background opacity-0 transition-opacity hover:scale-110 group-hover/cut:opacity-100"
        >
          <X class="size-2.5" />
        </button>
      </div>
    {/if}
  {/each}
</div>
