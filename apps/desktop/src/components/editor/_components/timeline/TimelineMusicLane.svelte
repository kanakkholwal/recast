<script lang="ts">
import {
	clipDisplayName,
	clipEndSec,
	moveClip,
	trimClipLeft,
	trimClipRight,
	type AudioClip,
} from "$lib/audio/music";
import type { EditorStore, PanelTab } from "$lib/stores/editor-store.svelte";
import { originalToOutput } from "$lib/timeline/time-map";
import { AudioLines, Repeat, Scissors, Trash2 } from "@recast/icons";
import { CLIP_ROW_HEIGHT_PX, edgeHandleWidth, type LaneCardLayout } from "./timeline-stack";

// Editable audio clips on the OUTPUT timeline: drag the body to move, the edges
// to trim, and split/delete a selected clip. Serves both the music lane and the
// detached-recording "voice" lane — same model, same store ops (keyed by id),
// only the subset (`clips`) and colour (`variant`) differ. The model, preview
// engine, and export all already read startOutputSec/offsetSec/durationSec.

interface Props {
	store: EditorStore;
	clips: AudioClip[];
	pixelsPerSecond: number;
	/** Card placement + lane height, computed by the timeline so the track rail
	 *  and this lane can never disagree on how tall the lane is. */
	layout: LaneCardLayout;
	variant?: "music" | "voice";
	panelTab?: PanelTab;
}
let {
	store,
	clips,
	pixelsPerSecond,
	layout,
	variant = "music",
	panelTab = "music",
}: Props = $props();

/** Below this the name can't fit legibly, so the card shows its icon. */
const NAME_WIDTH_PX = 56;
/** Keyboard move/trim step. Clips are music, not frames, so 100ms not 1/fps. */
const NUDGE_SEC = 0.1;
const outputDuration = $derived(store.timeMap.outputDuration);
const playheadOutput = $derived(originalToOutput(store.timeMap, store.currentTime));
const pps = $derived(pixelsPerSecond);

// Literal class strings per variant so Tailwind keeps them (no dynamic names).
// Same shape as the zoom and markup cards -- colour spine, tiered fill -- so
// every lane reads as one system in its own colour.
const cls = $derived(
	variant === "voice"
		? {
				bar: "border-l-lane-audio/70 bg-lane-audio/20 hover:bg-lane-audio/30",
				sel: "border-l-lane-audio bg-lane-audio/35 ring-1 ring-inset ring-lane-audio/70",
				icon: "text-lane-audio",
				handle: "bg-lane-audio/70",
			}
		: {
				bar: "border-l-lane-music/70 bg-lane-music/20 hover:bg-lane-music/30",
				sel: "border-l-lane-music bg-lane-music/35 ring-1 ring-inset ring-lane-music/70",
				icon: "text-lane-music",
				handle: "bg-lane-music/70",
			},
);

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
// Pushed on the first real move, not at pointer-down: clicking a clip to select
// it used to leave an undo entry that changed nothing.
let dragUndoPushed = false;

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
	dragUndoPushed = false;
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
	if (!dragUndoPushed) {
		store.pushUndoState();
		dragUndoPushed = true;
	}
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

// Arrow moves the clip, Alt+Arrow trims its end. Sequential presses coalesce
// into one undo entry so a held key is one edit.
function onClipKeydown(e: KeyboardEvent, clip: AudioClip) {
	if (e.key === "Enter" || e.key === " ") {
		e.preventDefault();
		store.selectMusicClip(clip.id);
		return;
	}
	if (e.key === "Delete" || e.key === "Backspace") {
		e.preventDefault();
		e.stopPropagation();
		store.removeMusicClip(clip.id);
		return;
	}
	if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
	e.preventDefault();
	e.stopPropagation();
	store.selectMusicClip(clip.id);
	store.pushUndoStateCoalesced(`clip-${clip.id}`, 600);
	const step = (e.shiftKey ? 1 : NUDGE_SEC) * (e.key === "ArrowLeft" ? -1 : 1);
	const next = e.altKey
		? trimClipRight(clip, clipEndSec(clip, outputDuration) + step, outputDuration)
		: moveClip(clip, clip.startOutputSec + step, outputDuration);
	store.updateMusicClip(clip.id, next);
}
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointermove={onMove}
  onpointerup={endDrag}
  onpointercancel={endDrag}
  class="relative mt-1.5 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5 transition-[height]"
  style="height: {layout.height}px;"
>
  {#each clips as clip, i (clip.id)}
    {@const card = layout.cards[i]}
    {@const selected = store.selectedMusicClipId === clip.id}
    {@const gripPx = edgeHandleWidth(card.width)}
    <div
      role="button"
      tabindex="0"
      aria-label={`${clipDisplayName(clip)}. Drag to move; drag the edges to trim; Alt+Arrow trims the end.`}
      title={clipDisplayName(clip)}
      onpointerdown={(e) => startDrag(e, clip, "move")}
      ondblclick={() => (store.activePanel = panelTab)}
      onkeydown={(e) => onClipKeydown(e, clip)}
      class="group/clip absolute flex touch-none items-center gap-1 overflow-hidden rounded-[3px] border-l-2 px-1.5 text-[10px] text-foreground transition-colors duration-150 focus:outline-none focus:ring-1 focus:ring-inset focus:ring-ring {selected
        ? cls.sel
        : cls.bar} {clip.muted ? 'opacity-50' : ''} {drag ? 'cursor-grabbing' : 'cursor-grab'}"
      style="left: {card.left}px; width: {card.width}px; top: {card.top}px; height: {CLIP_ROW_HEIGHT_PX}px;"
    >
      {#if card.width < NAME_WIDTH_PX}
        <AudioLines size={10} class="pointer-events-none shrink-0 {cls.icon}" />
      {:else}
        <span class="pointer-events-none truncate font-medium">{clipDisplayName(clip)}</span>
        {#if clip.loop}
          <Repeat size={9} class="pointer-events-none ml-auto shrink-0 opacity-70" />
        {/if}
      {/if}

      <!-- Pointer-only grips, always present so trimming doesn't need a select
           click first. They used to be <button>s nested in this role="button"
           with no key handler: focusable, announced, and inert. Alt+Arrow on the
           clip is the keyboard route. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        aria-hidden="true"
        onpointerdown={(e) => startDrag(e, clip, "trim-left")}
        class="absolute inset-y-0 left-0 cursor-ew-resize"
        style="width: {gripPx}px;"
      >
        <div
          class="mx-auto h-full w-0.5 rounded-l-sm opacity-0 transition-opacity group-hover/clip:opacity-100 {cls.handle} {selected
            ? 'opacity-100!'
            : ''}"
        ></div>
      </div>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        aria-hidden="true"
        onpointerdown={(e) => startDrag(e, clip, "trim-right")}
        class="absolute inset-y-0 right-0 cursor-ew-resize"
        style="width: {gripPx}px;"
      >
        <div
          class="ml-auto h-full w-0.5 rounded-r-sm opacity-0 transition-opacity group-hover/clip:opacity-100 {cls.handle} {selected
            ? 'opacity-100!'
            : ''}"
        ></div>
      </div>
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
