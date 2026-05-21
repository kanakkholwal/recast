<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import type { TimelineCut } from "$lib/timeline/cuts";
  import { Eye, EyeOff, Scissors, X } from "@lucide/svelte";

  // Lane that hosts cut bands — the ranges removed from the timeline.
  // Drag empty lane space to carve a new cut; drag a band's edges to
  // fine-tune it, or its body to slide it. Mirrors the zoom/annotation
  // lanes structurally so the timeline reads consistently.

  interface Props {
    store: EditorStore;
    pixelsPerSecond: number;
    duration: number;
  }

  let { store, pixelsPerSecond, duration }: Props = $props();

  // Cuts shorter than this are dropped — a sub-100ms removal reads as a
  // glitch, not an edit.
  const MIN_CUT = 0.1;

  let laneEl = $state<HTMLDivElement | null>(null);

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
    return Math.min(duration, Math.max(0, x / pixelsPerSecond));
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
    if (drag.mode === "move") {
      let s = drag.originStart + delta;
      let en = drag.originEnd + delta;
      if (s < 0) {
        en -= s;
        s = 0;
      }
      if (en > duration) {
        s -= en - duration;
        en = duration;
      }
      store.updateCut(drag.id, Math.max(0, s), en);
    } else {
      let s = drag.originStart;
      let en = drag.originEnd;
      if (drag.mode === "resize-l") {
        s = Math.min(Math.max(0, drag.originStart + delta), en - MIN_CUT);
      } else {
        en = Math.max(
          Math.min(duration, drag.originEnd + delta),
          s + MIN_CUT,
        );
      }
      store.updateCut(drag.id, s, en);
    }
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

  // Filled peak envelope, drawn behind the cut bands so the user can see the
  // audio they're cutting against. `viewBox` units are bucket-indexed and the
  // SVG is stretched to the timeline width, so it never needs a redraw on
  // zoom — only when the waveform data itself changes.
  const waveformPath = $derived.by(() => {
    const w = store.waveform;
    if (w.length < 2) return "";
    let d = "M 0 50";
    for (let i = 0; i < w.length; i++) {
      d += ` L ${i} ${(50 - w[i] * 46).toFixed(2)}`;
    }
    for (let i = w.length - 1; i >= 0; i--) {
      d += ` L ${i} ${(50 + w[i] * 46).toFixed(2)}`;
    }
    return d + " Z";
  });
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
      style="width: {duration * pixelsPerSecond}px;"
      viewBox="0 0 {store.waveform.length} 100"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <path d={waveformPath} class="fill-foreground/20" />
    </svg>
  {/if}

  <div
    class="pointer-events-none sticky left-1.5 top-1 z-50 inline-flex w-fit items-center gap-1"
  >
    <span
      class="inline-flex items-center gap-1 rounded-sm bg-destructive/15 px-1.5 py-px font-mono text-[8px] font-bold uppercase tracking-wider text-destructive backdrop-blur-sm"
    >
      <Scissors class="size-2" />
      Cuts
    </span>
    <button
      type="button"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={() => (store.cutsEnabled = !store.cutsEnabled)}
      title={store.cutsEnabled
        ? "Disable cuts (cuts stay; playback & export ignore them)"
        : "Enable cuts"}
      aria-label={store.cutsEnabled ? "Disable cuts" : "Enable cuts"}
      class="pointer-events-auto flex size-4 items-center justify-center rounded text-muted-foreground hover:bg-muted/60 hover:text-foreground"
    >
      {#if store.cutsEnabled}
        <Eye class="size-2.5" />
      {:else}
        <EyeOff class="size-2.5" />
      {/if}
    </button>
  </div>

  {#if store.cuts.length === 0}
    <div
      class="pointer-events-none flex h-6 items-center justify-center text-[10px] text-muted-foreground"
    >
      Drag across this lane to remove a section
    </div>
  {/if}

  {#each store.cuts as cut (cut.id)}
    {@const w = Math.max(8, (cut.end - cut.start) * pixelsPerSecond)}
    <div
      role="presentation"
      onpointerdown={(e) => onBandDown(e, cut, "move")}
      title="Removed section · {(cut.end - cut.start).toFixed(2)}s"
      class="group/cut absolute top-1.5 bottom-1.5 cursor-grab overflow-hidden rounded-sm border border-destructive/50 bg-destructive/20 active:cursor-grabbing"
      style="left: {cut.start * pixelsPerSecond}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--destructive) 22%, transparent) 5px, color-mix(in srgb, var(--destructive) 22%, transparent) 10px);"
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
  {/each}
</div>
