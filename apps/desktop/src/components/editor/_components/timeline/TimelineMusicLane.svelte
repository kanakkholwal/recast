<script lang="ts">
  import {
    clipDisplayName,
    clipEndSec,
    clipPlaySec,
    moveClip,
    trimClipLeft,
    trimClipRight,
    type AudioClip,
  } from "$lib/audio/music";
  import type { EditorStore, PanelTab } from "$lib/stores/editor-store.svelte";
  import { originalToOutput } from "$lib/timeline/time-map";
  import { AudioLines, Repeat, Scissors, Trash2 } from "@recast/icons";

  // Editable audio clips on the OUTPUT timeline: drag the body to move, the edges
  // to trim, and split/delete a selected clip. Serves both the music lane and the
  // detached-recording "voice" lane — same model, same store ops (keyed by id),
  // only the subset (`clips`) and colour (`variant`) differ. The model, preview
  // engine, and export all already read startOutputSec/offsetSec/durationSec.

  interface Props {
    store: EditorStore;
    clips: AudioClip[];
    pixelsPerSecond: number;
    variant?: "music" | "voice";
    panelTab?: PanelTab;
  }
  let {
    store,
    clips,
    pixelsPerSecond,
    variant = "music",
    panelTab = "music",
  }: Props = $props();

  const LANE_H = 28;
  const outputDuration = $derived(store.timeMap.outputDuration);
  const playheadOutput = $derived(originalToOutput(store.timeMap, store.currentTime));
  const pps = $derived(pixelsPerSecond);

  // Literal class strings per variant so Tailwind keeps them (no dynamic names).
  const cls = $derived(
    variant === "voice"
      ? {
          bar: "border-lane-audio/40 bg-lane-audio/15 hover:bg-lane-audio/25",
          sel: "border-lane-audio ring-1 ring-lane-audio/60",
          icon: "text-lane-audio",
          handle: "bg-lane-audio/70 hover:bg-lane-audio",
        }
      : {
          bar: "border-lane-music/40 bg-lane-music/15 hover:bg-lane-music/25",
          sel: "border-lane-music ring-1 ring-lane-music/60",
          icon: "text-lane-music",
          handle: "bg-lane-music/70 hover:bg-lane-music",
        },
  );

  // Clips live on the OUTPUT axis; the lane renders on the render axis. Convert
  // output seconds → render-axis pixels (identity unless "Show cut gaps" is on).
  const xPx = (outputSec: number) => store.outputToRenderSec(outputSec) * pps;

  function bar(clip: AudioClip) {
    const left = xPx(clip.startOutputSec);
    return { left, width: Math.max(3, xPx(clipEndSec(clip, outputDuration)) - left) };
  }

  let laneEl = $state<HTMLDivElement | null>(null);

  type DragMode = "move" | "trim-left" | "trim-right";
  interface Drag {
    pointerId: number;
    id: string;
    mode: DragMode;
    /** Body drag: pointer's offset from the clip start, so the grab point holds. */
    grabSec: number;
  }
  let drag = $state<Drag | null>(null);

  function outputSecAt(clientX: number): number {
    if (!laneEl || pps <= 0) return 0;
    const x = clientX - laneEl.getBoundingClientRect().left;
    return Math.max(0, store.renderSecToOutputSec(x / pps));
  }

  // Snap the dragged edge/position to the playhead and the timeline ends.
  function snap(sec: number): number {
    const tol = pps > 0 ? 6 / pps : 0;
    for (const target of [playheadOutput, 0, outputDuration]) {
      if (Math.abs(sec - target) <= tol) return target;
    }
    return sec;
  }

  function startDrag(e: PointerEvent, clip: AudioClip, mode: DragMode) {
    if (e.button !== 0 || store.timelineTool === "razor") return;
    e.preventDefault();
    e.stopPropagation();
    store.selectMusicClip(clip.id);
    store.pushUndoState();
    drag = {
      pointerId: e.pointerId,
      id: clip.id,
      mode,
      grabSec: outputSecAt(e.clientX) - clip.startOutputSec,
    };
    laneEl?.setPointerCapture(e.pointerId);
  }

  function onMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const clip = store.musicClips.find((c) => c.id === drag!.id);
    if (!clip) return;
    const at = outputSecAt(e.clientX);
    if (drag.mode === "move") {
      store.updateMusicClip(clip.id, moveClip(clip, snap(at - drag.grabSec), outputDuration));
    } else if (drag.mode === "trim-left") {
      store.updateMusicClip(clip.id, trimClipLeft(clip, snap(at), outputDuration));
    } else {
      store.updateMusicClip(clip.id, trimClipRight(clip, snap(at), outputDuration));
    }
  }

  function endDrag(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    laneEl?.releasePointerCapture(e.pointerId);
    drag = null;
  }

  const selectedHere = $derived(clips.some((c) => c.id === store.selectedMusicClipId));
  const canSplitSelected = $derived.by(() => {
    const id = store.selectedMusicClipId;
    if (!id || !selectedHere) return false;
    const clip = clips.find((c) => c.id === id);
    if (!clip) return false;
    return (
      playheadOutput > clip.startOutputSec + 0.1 &&
      playheadOutput < clipEndSec(clip, outputDuration) - 0.1
    );
  });

  function splitSelected() {
    if (store.selectedMusicClipId) store.splitMusicClip(store.selectedMusicClipId, playheadOutput);
  }
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointermove={onMove}
  onpointerup={endDrag}
  onpointercancel={endDrag}
  class="relative mt-1.5 rounded-md border border-border/60 bg-background/40"
  style="height: {LANE_H}px;"
>
  {#each clips as clip (clip.id)}
    {@const b = bar(clip)}
    {@const selected = store.selectedMusicClipId === clip.id}
    <div
      role="button"
      tabindex="0"
      aria-label={clipDisplayName(clip)}
      title={clipDisplayName(clip)}
      onpointerdown={(e) => startDrag(e, clip, "move")}
      ondblclick={() => (store.activePanel = panelTab)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          store.selectMusicClip(clip.id);
        }
      }}
      class="group absolute inset-y-1 flex touch-none items-center gap-1 overflow-hidden rounded border px-1.5 text-[10px] text-foreground transition-[background-color] {selected
        ? cls.sel
        : cls.bar} {clip.muted ? 'opacity-50' : ''} {drag ? 'cursor-grabbing' : 'cursor-grab'}"
      style="left: {b.left}px; width: {b.width}px;"
    >
      <AudioLines size={10} class="pointer-events-none shrink-0 {cls.icon}" />
      <span class="pointer-events-none truncate">{clipDisplayName(clip)}</span>
      {#if clip.loop}
        <Repeat size={9} class="pointer-events-none ml-auto shrink-0 opacity-70" />
      {/if}

      <!-- Trim handles: only on the selected clip, so idle bars stay clean. -->
      {#if selected}
        <button
          type="button"
          aria-label="Trim start"
          class="absolute inset-y-0 left-0 w-1.5 cursor-ew-resize rounded-l {cls.handle}"
          onpointerdown={(e) => startDrag(e, clip, "trim-left")}
        ></button>
        <button
          type="button"
          aria-label="Trim end"
          class="absolute inset-y-0 right-0 w-1.5 cursor-ew-resize rounded-r {cls.handle}"
          onpointerdown={(e) => startDrag(e, clip, "trim-right")}
        ></button>
      {/if}
    </div>
  {/each}

  <!-- Action rail for the selected clip (only in the lane that holds it). -->
  {#if selectedHere}
    <div class="absolute -top-0.5 right-1 z-10 flex -translate-y-full items-center gap-0.5">
      <button
        type="button"
        aria-label="Split at playhead"
        title="Split at playhead"
        disabled={!canSplitSelected}
        class="rounded bg-popover p-1 text-muted-foreground shadow-sm hover:text-foreground disabled:opacity-40"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={splitSelected}
      >
        <Scissors size={12} />
      </button>
      <button
        type="button"
        aria-label="Delete clip"
        title="Delete clip"
        class="rounded bg-popover p-1 text-muted-foreground shadow-sm hover:text-destructive"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={() => store.selectedMusicClipId && store.removeMusicClip(store.selectedMusicClipId)}
      >
        <Trash2 size={12} />
      </button>
    </div>
  {/if}
</div>
