<script lang="ts">
import {
	clipDisplayName,
	clipEndSec,
	moveClip,
	trimClipLeft,
	trimClipRight,
	type AudioClip,
} from "../../../lib/audio/music";
import type { EditorStore, PanelTab } from "../../../stores/editor-store.svelte";
import { originalToOutput } from "../../../lib/timeline/time-map";
import { AudioLines, Repeat, Scissors, Trash2 } from "@recast/icons";
import { dragEngaged, PRECISION_SCALE } from "./timeline-card-drag.logic";
import {
	CLIP_BASE,
	CLIP_FOCUS,
	CLIP_HOVER,
	CLIP_LABEL,
	CLIP_SELECTED,
	clipSurface,
} from "./timeline-clip.styles";
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

// Same solid-body treatment as every other lane, in this lane's hue.
const surface = $derived(clipSurface(variant === "voice" ? "audio" : "music"));

let laneEl = $state<HTMLDivElement | null>(null);

type DragMode = "move" | "trim-left" | "trim-right";
interface Drag {
	pointerId: number;
	id: string;
	mode: DragMode;
	/** Body drag: pointer's offset from the clip start, so the grab point holds. */
	grabSec: number;
	startClientX: number;
	/** False until the pointer clears the drag threshold. */
	engaged: boolean;
	/** Shift held: pointer travel is damped for fine positioning. */
	precision: boolean;
	/** Output second the precision gearing is measured from. */
	precisionAnchor: number;
	/** Keeps the geared position continuous across a modifier flip. */
	gearOffset: number;
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
// `bypass` is Ctrl/Cmd held, for a placement the magnetism fights.
function snap(sec: number, bypass = false): number {
	if (bypass) return sec;
	const tol = pps > 0 ? 6 / pps : 0;
	for (const target of [playheadOutput, 0, outputDuration]) {
		if (Math.abs(sec - target) <= tol) return target;
	}
	return sec;
}

function gearedValue(raw: number): number {
	if (!drag) return raw;
	const base = drag.precision
		? drag.precisionAnchor + (raw - drag.precisionAnchor) * PRECISION_SCALE
		: raw;
	return base + drag.gearOffset;
}

// Pointer position in output seconds, damped while Shift is held. On a modifier
// flip the anchor and offset are re-seeded so `gearedValue` is continuous —
// otherwise letting go of Shift would teleport the clip to the raw pointer.
function gearedSecAt(event: PointerEvent): number {
	if (!drag) return 0;
	const raw = outputSecAt(event.clientX);
	if (event.shiftKey !== drag.precision) {
		const before = gearedValue(raw);
		drag.precision = event.shiftKey;
		drag.precisionAnchor = raw;
		drag.gearOffset = before - raw;
	}
	return gearedValue(raw);
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
		startClientX: e.clientX,
		engaged: false,
		precision: e.shiftKey,
		precisionAnchor: outputSecAt(e.clientX),
		gearOffset: 0,
	};
	laneEl?.setPointerCapture(e.pointerId);
}

function onMove(e: PointerEvent) {
	if (!drag || e.pointerId !== drag.pointerId) return;
	// A press is a click until it clears the threshold, so selecting a clip
	// can't nudge it or leave an undo entry that changed nothing.
	if (!drag.engaged) {
		if (!dragEngaged(e.clientX, drag.startClientX)) return;
		drag.engaged = true;
	}
	const clip = store.musicClips.find((c) => c.id === drag!.id);
	if (!clip) return;
	if (!dragUndoPushed) {
		store.pushUndoState();
		dragUndoPushed = true;
	}
	const at = gearedSecAt(e);
	const bypass = e.ctrlKey || e.metaKey;
	if (drag.mode === "move") {
		store.updateMusicClip(clip.id, moveClip(clip, snap(at - drag.grabSec, bypass), outputDuration));
	} else if (drag.mode === "trim-left") {
		store.updateMusicClip(clip.id, trimClipLeft(clip, snap(at, bypass), outputDuration));
	} else {
		store.updateMusicClip(clip.id, trimClipRight(clip, snap(at, bypass), outputDuration));
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
  class="relative mt-1.5 rounded-md bg-muted/20 px-1.5 py-1.5 transition-[height]"
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
      class="group/clip absolute flex touch-none items-center gap-1 px-1.5 {CLIP_BASE} {CLIP_FOCUS} {surface.fill} {selected
        ? CLIP_SELECTED
        : CLIP_HOVER} {clip.muted ? 'opacity-50' : ''} {drag
        ? 'cursor-grabbing'
        : 'cursor-grab'}"
      style="left: {card.left}px; width: {card.width}px; top: {card.top}px; height: {CLIP_ROW_HEIGHT_PX}px;"
    >
      {#if card.width < NAME_WIDTH_PX}
        <AudioLines size={10} class="pointer-events-none mx-auto shrink-0 {surface.accent} opacity-60" />
      {:else}
        <span class={CLIP_LABEL}>{clipDisplayName(clip)}</span>
        {#if clip.loop}
          <Repeat size={9} class="pointer-events-none ml-auto shrink-0 {surface.accent} opacity-70" />
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
          class="mx-auto h-full w-0.5 rounded-l-sm opacity-0 transition-opacity group-hover/clip:opacity-100 {surface.grip} {selected
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
          class="ml-auto h-full w-0.5 rounded-r-sm opacity-0 transition-opacity group-hover/clip:opacity-100 {surface.grip} {selected
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
