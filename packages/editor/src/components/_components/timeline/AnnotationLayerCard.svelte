<script lang="ts">
import { kindIcon, kindLabel } from "../../../lib/annotations/kind-label";
import type { Annotation, EditorStore } from "../../../stores/editor-store.svelte";
import { originalToOutput, outputToOriginal } from "../../../lib/timeline/time-map";
import { motionDuration } from "../../../lib/motion.svelte";

import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";
import {
	computeCardMove,
	computeCardNudge,
	computeCardResize,
	dragEngaged,
	PRECISION_SCALE,
} from "./timeline-card-drag.logic";
import {
	CLIP_BASE,
	CLIP_FOCUS,
	CLIP_HOVER,
	CLIP_LABEL,
	CLIP_META,
	CLIP_SELECTED,
	clipSurface,
} from "./timeline-clip.styles";
import { useLaneDrag } from "./timeline-drag.svelte";
import { formatTimeByMode, type TimeMode } from "./timeline-helpers";
import { type SnapResult, type SnapTarget } from "./timeline-snap";
import { EDGE_HIT_OVERHANG_PX, edgeHandleWidth, ROW_HEIGHT_PX } from "./timeline-stack";

// Mirrors ZoomLayerCard's drag/resize/snap on annotations; outline-only
// (no sparkline) so the two lanes are distinguishable at a glance.

interface Props {
	store: EditorStore;
	annotation: Annotation;
	pixelsPerSecond: number;
	fps: number;
	duration: number;
	/** Layout comes from the lane, which packs overlapping cards into rows. */
	left: number;
	width: number;
	top: number;
	snapTargets: SnapTarget[];
	timeMode: TimeMode;
	onSnapChange: (snap: SnapResult["target"] | null) => void;
	onDuplicate: (annotation: Annotation) => void;
}

let {
	store,
	annotation,
	pixelsPerSecond,
	fps,
	duration,
	left,
	width,
	top,
	snapTargets,
	timeMode,
	onSnapChange,
	onDuplicate,
}: Props = $props();

const MIN_DURATION = 0.05; // Annotations can be tighter than zooms.
const SNAP_TOLERANCE_PX = 6;
/** Below this the name can't fit legibly, so the card shows its kind icon. */
const NAME_WIDTH_PX = 56;

type DragMode = "move" | "resize-start" | "resize-end";

interface DragContext {
	mode: DragMode;
	pointerId: number;
	startClientX: number;
	originalStart: number;
	originalEnd: number;
	/** False until the pointer clears the drag threshold. */
	engaged: boolean;
	/** Shift held: pointer travel is damped for fine positioning. */
	precision: boolean;
}

let drag = $state<DragContext | null>(null);
// Undo is pushed on the first real move, not at pointer-down: clicking a card
// to select it used to leave an undo entry that changed nothing, so Ctrl+Z
// after selecting five cards did nothing five times.
let dragUndoPushed = false;
// Holds this card's row for the gesture, so re-packing can't move it off the cursor.
const laneDrag = useLaneDrag();

const isSelected = $derived(annotation.id === store.selectedAnnotationId);
// Output (post-cut) axis. See ZoomLayerCard for the rationale.
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
const tOf = (xPx: number) => outputToOriginal(store.renderMap, xPx / pixelsPerSecond);
// Labels read on the output axis, like the ruler and the playhead.
const outSec = (t: number) => originalToOutput(store.renderMap, t);
const showSubtitle = $derived(width >= 110);
const handlePx = $derived(edgeHandleWidth(width));
const Icon = $derived(kindIcon(annotation));
const surface = clipSurface("markup");

function beginDrag(mode: DragMode, event: PointerEvent) {
	if (duration <= 0) return;
	// Let a razor click bubble through to carve, rather than dragging the card.
	if (store.timelineTool === "razor") return;
	event.preventDefault();
	event.stopPropagation();
	store.selectedAnnotationId = annotation.id;
	dragUndoPushed = false;
	laneDrag?.begin(annotation.id);
	drag = {
		mode,
		pointerId: event.pointerId,
		startClientX: event.clientX,
		originalStart: annotation.start,
		originalEnd: annotation.end,
		engaged: false,
		precision: event.shiftKey,
	};
	document.body.style.cursor = mode === "move" ? "grabbing" : "ew-resize";
	(event.currentTarget as Element).setPointerCapture(event.pointerId);
	window.addEventListener("pointermove", onPointerMove);
	window.addEventListener("pointerup", onPointerUp);
	window.addEventListener("pointercancel", onPointerUp);
}

function onPointerMove(event: PointerEvent) {
	if (!drag) return;
	// A press is a click until it clears the threshold, so selecting a card
	// can't nudge it or leave an undo entry that changed nothing.
	if (!drag.engaged) {
		if (!dragEngaged(event.clientX, drag.startClientX)) return;
		drag.engaged = true;
	}
	// Shift can go down or up mid-drag; re-seed the anchor to the current
	// pointer and bounds so the change in gearing never jumps the card.
	if (event.shiftKey !== drag.precision) {
		drag.precision = event.shiftKey;
		drag.startClientX = event.clientX;
		drag.originalStart = annotation.start;
		drag.originalEnd = annotation.end;
	}
	if (!dragUndoPushed) {
		store.pushUndoState();
		dragUndoPushed = true;
	}
	const geom = {
		origin: { start: drag.originalStart, end: drag.originalEnd },
		clientX: event.clientX,
		startClientX: drag.startClientX,
		xOf,
		tOf,
		// Ctrl/Cmd suspends magnetism for a placement the snap targets fight.
		snapTargets: event.ctrlKey || event.metaKey ? [] : snapTargets,
		tolerance: SNAP_TOLERANCE_PX / pixelsPerSecond,
		fps,
		duration,
		scale: drag.precision ? PRECISION_SCALE : 1,
	};
	const result =
		drag.mode === "move"
			? computeCardMove(geom)
			: computeCardResize({
					...geom,
					edge: drag.mode === "resize-start" ? "start" : "end",
					minDuration: MIN_DURATION,
				});
	store.updateAnnotation(annotation.id, {
		start: result.start,
		end: result.end,
	});
	onSnapChange(result.guide);
}

function onPointerUp(_event: PointerEvent) {
	drag = null;
	laneDrag?.end();
	document.body.style.cursor = "";
	window.removeEventListener("pointermove", onPointerMove);
	window.removeEventListener("pointerup", onPointerUp);
	window.removeEventListener("pointercancel", onPointerUp);
	onSnapChange(null);
}

function onCardKeydown(event: KeyboardEvent) {
	if (duration <= 0) return;

	if (event.key === "Delete" || event.key === "Backspace") {
		event.preventDefault();
		event.stopPropagation();
		store.removeAnnotation(annotation.id);
		return;
	}

	const isMod = event.ctrlKey || event.metaKey;
	if (isMod && (event.key === "d" || event.key === "D")) {
		event.preventDefault();
		event.stopPropagation();
		onDuplicate(annotation);
		return;
	}

	if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
	event.preventDefault();
	event.stopPropagation();

	store.pushUndoStateCoalesced(`nudge-annotation-${annotation.id}`, 600);

	const next = computeCardNudge({
		origin: { start: annotation.start, end: annotation.end },
		direction: event.key === "ArrowLeft" ? -1 : 1,
		shift: event.shiftKey,
		alt: event.altKey,
		fps,
		duration,
		minDuration: MIN_DURATION,
	});
	store.updateAnnotation(annotation.id, {
		start: next.start,
		end: next.end,
	});
}

function onCardClick(event: MouseEvent) {
	if (store.timelineTool === "razor") return; // razor click is not a select
	event.stopPropagation();
	store.selectedAnnotationId = annotation.id;
}
</script>

<div
  in:fly={{ y: 10, duration: motionDuration(180), easing: cubicOut }}
  out:fade={{ duration: motionDuration(140) }}
  class="group/card absolute z-20 overflow-visible select-none"
  style="
    left: {left}px;
    width: {width}px;
    top: {top}px;
    height: {ROW_HEIGHT_PX}px;
  "
>
  <button
    type="button"
    aria-pressed={isSelected}
    onclick={onCardClick}
    onkeydown={onCardKeydown}
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("move", e);
    }}
    class="absolute inset-0 text-left {CLIP_BASE} {CLIP_FOCUS} {surface.fill} {isSelected
      ? `${CLIP_SELECTED} cursor-grabbing`
      : `${CLIP_HOVER} cursor-grab`} {drag?.mode === 'move'
      ? 'cursor-grabbing shadow-craft-floating brightness-110'
      : ''}"
  >
    <!-- Content by available width, the way an NLE clip degrades: icon only when
         narrow, then the name, then the length. The old card always rendered a
         20px icon tile AND the name AND a timecode, which on a short card left
         nothing but a clipped icon. -->
    <div
      class="pointer-events-none flex h-full items-center gap-1 px-1.5 {width < NAME_WIDTH_PX
        ? 'justify-center'
        : ''}"
      id={`annotation-region-${annotation.id}`}
      aria-label={`${kindLabel(annotation)} annotation from ${formatTimeByMode(outSec(annotation.start), timeMode, fps)} to ${formatTimeByMode(outSec(annotation.end), timeMode, fps)}. Click to select; drag to move; drag the edges to resize.`}
    >
      {#if width < NAME_WIDTH_PX}
        <!-- Too narrow for a label; the glyph sits back so a short annotation
             reads as a clip, not a button. -->
        <Icon class="size-3 shrink-0 {surface.accent} opacity-60" />
      {:else}
        <span class={CLIP_LABEL}>{kindLabel(annotation)}</span>
        {#if showSubtitle}
          <span class="ml-auto {CLIP_META}">
            {formatTimeByMode(outSec(annotation.end) - outSec(annotation.start), timeMode, fps)}
          </span>
        {/if}
      {/if}
    </div>
  </button>

  <!-- Pointer-only grips. They used to carry role="slider" with tabindex="-1"
       and no key handler: announced as sliders, impossible to focus or operate.
       Keyboard resize lives on the card itself (Alt+Arrow). Width scales with
       the card so a short one keeps more middle to drag than edge to resize. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    aria-hidden="true"
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-start", e);
    }}
    class="absolute inset-y-0 z-10 cursor-ew-resize"
    style="width: {handlePx + EDGE_HIT_OVERHANG_PX}px; left: -{EDGE_HIT_OVERHANG_PX}px;"
  >
    <div
      class="mx-auto h-full w-0.5 rounded-l-sm {surface.grip} opacity-0 transition-opacity group-hover/card:opacity-100 {isSelected ||
      drag?.mode === 'resize-start'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    aria-hidden="true"
    onpointerdown={(e) => {
      if (e.button !== 0) return;
      beginDrag("resize-end", e);
    }}
    class="absolute inset-y-0 z-10 cursor-ew-resize"
    style="width: {handlePx + EDGE_HIT_OVERHANG_PX}px; right: -{EDGE_HIT_OVERHANG_PX}px;"
  >
    <div
      class="ml-auto h-full w-0.5 rounded-r-sm {surface.grip} opacity-0 transition-opacity group-hover/card:opacity-100 {isSelected ||
      drag?.mode === 'resize-end'
        ? 'opacity-100!'
        : ''}"
    ></div>
  </div>
</div>
