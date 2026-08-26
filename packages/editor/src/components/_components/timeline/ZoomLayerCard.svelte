<script lang="ts">
import { ZoomIn } from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";
import { motionDuration } from "../../../lib/motion.svelte";
import { originalToOutput, outputToOriginal } from "../../../lib/timeline/time-map";
import type { EditorStore, ZoomRegion } from "../../../stores/editor-store.svelte";
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
import { EDGE_HIT_OVERHANG_PX, edgeHandleWidth, ZOOM_ROW_HEIGHT_PX } from "./timeline-stack";

// Three drag modes through one pointer-handler: move (shift both edges),
// resize-start (move `start`), resize-end (move `end`). Undo is pushed on the
// first real move so a select-click leaves no empty history entry.

interface Props {
	store: EditorStore;
	region: ZoomRegion;
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
	onCopy: (region: ZoomRegion) => void;
	onDuplicate: (region: ZoomRegion) => void;
}

let {
	store,
	region,
	pixelsPerSecond,
	fps,
	duration,
	left,
	width,
	top,
	snapTargets,
	timeMode,
	onSnapChange,
	onCopy,
	onDuplicate,
}: Props = $props();

// Floor so a card can't collapse to zero width (0.1s ≈ 6 frames at 60fps).
const MIN_DURATION = 0.1;

const SNAP_TOLERANCE_PX = 6;
/** Below this the label can't fit legibly, so the card shows its icon. */
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
let dragUndoPushed = false;
// Holds this card's row for the gesture, so re-packing can't move it off the cursor.
const laneDrag = useLaneDrag();

const isSelected = $derived(region.id === store.selectedZoomRegionId);
// Output (post-cut) axis so regions sit on the same gapless line as clips;
// a region overlapping a cut renders narrower (correct NLE behaviour).
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
const tOf = (xPx: number) => outputToOriginal(store.renderMap, xPx / pixelsPerSecond);
// Labels read on the output axis, like the ruler and the playhead. Regions are
// STORED in original time, so printing that raw would name a timecode the
// exported file never reaches once anything upstream is cut.
const outSec = (t: number) => originalToOutput(store.renderMap, t);
const showSubtitle = $derived(width >= 110);
const handlePx = $derived(edgeHandleWidth(width));
const surface = clipSurface("zoom");

function beginDrag(mode: DragMode, event: PointerEvent) {
	if (duration <= 0) return;
	// Let a razor click bubble through to carve, rather than dragging the card.
	if (store.timelineTool === "razor") return;
	event.preventDefault();
	event.stopPropagation();
	store.selectedZoomRegionId = region.id;
	dragUndoPushed = false;
	laneDrag?.begin(region.id);
	drag = {
		mode,
		pointerId: event.pointerId,
		startClientX: event.clientX,
		originalStart: region.start,
		originalEnd: region.end,
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
		drag.originalStart = region.start;
		drag.originalEnd = region.end;
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
	store.updateZoomRegion(region.id, { start: result.start, end: result.end });
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

// Coalesces sequential nudges into one undo entry so a held arrow is one edit.
function onCardKeydown(event: KeyboardEvent) {
	if (duration <= 0) return;

	// Delete is owned by the editor page and acts on the selection (this card is
	// the selection whenever it has focus), so it is deliberately not handled here.

	// Paste lives at timeline scope so regions land at the playhead, not here.
	const isMod = event.ctrlKey || event.metaKey;
	if (isMod && (event.key === "d" || event.key === "D")) {
		event.preventDefault();
		event.stopPropagation();
		onDuplicate(region);
		return;
	}
	if (isMod && (event.key === "c" || event.key === "C")) {
		event.preventDefault();
		event.stopPropagation();
		onCopy(region);
		return;
	}

	if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
	event.preventDefault();
	event.stopPropagation();

	store.pushUndoStateCoalesced(`nudge-zoom-${region.id}`, 600);

	const next = computeCardNudge({
		origin: { start: region.start, end: region.end },
		direction: event.key === "ArrowLeft" ? -1 : 1,
		shift: event.shiftKey,
		alt: event.altKey,
		fps,
		duration,
		minDuration: MIN_DURATION,
	});
	store.updateZoomRegion(region.id, { start: next.start, end: next.end });
}

function onCardClick(event: MouseEvent) {
	// A real drag never fires this (window-level pointer handlers); only a static click does.
	if (store.timelineTool === "razor") return; // razor click is not a select
	event.stopPropagation();
	store.selectedZoomRegionId = region.id;
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
    height: {ZOOM_ROW_HEIGHT_PX}px;
  "
>
  <!-- Body split from the resize edges so each gets its own cursor. Solid fill
       with the label inside it, like an NLE clip; the bright lane accent is left
       to the grips and the icon so the block reads as fill + accent. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <button
    type="button"
    aria-pressed={isSelected}
    onclick={onCardClick}
    onkeydown={onCardKeydown}
    onpointerdown={(e) => {
      // Resize handlers stop propagation, so anything reaching here is a body drag.
      if (e.button !== 0) return;
      beginDrag("move", e);
    }}
    class="absolute inset-0 text-left {CLIP_BASE} {CLIP_FOCUS} {surface.fill} {isSelected
      ? `${CLIP_SELECTED} cursor-grabbing`
      : `${CLIP_HOVER} cursor-grab`} {drag?.mode === 'move'
      ? 'cursor-grabbing shadow-craft-floating brightness-110'
      : ''}"
  >
    <!-- Content by available width, like an NLE clip: icon when narrow, then the
         scale, then the length. Same tiers as the markup card so the two lanes
         read as one system in different colours. -->
    <div
      class="pointer-events-none flex h-full items-center gap-1 px-1.5 {width < NAME_WIDTH_PX
        ? 'justify-center'
        : ''}"
      id={`zoom-region-${region.id}`}
      aria-label={`Focus region from ${formatTimeByMode(outSec(region.start), timeMode, fps)} to ${formatTimeByMode(outSec(region.end), timeMode, fps)}, scale ${region.scale.toFixed(1)}x. Click to select; drag to move; drag the edges to resize.`}
    >
      {#if width < NAME_WIDTH_PX}
        <!-- Too narrow for a label. The fill already says which lane this is, so
             the glyph sits back at 60% rather than turning a short region into a
             high-contrast button. -->
        <ZoomIn class="size-3 shrink-0 {surface.accent} opacity-60" />
      {:else}
        <span class={CLIP_LABEL}>{region.scale.toFixed(1)}× zoom</span>
        {#if showSubtitle}
          <span class="ml-auto {CLIP_META}">
            {formatTimeByMode(outSec(region.end) - outSec(region.start), timeMode, fps)}
          </span>
        {/if}
      {/if}
    </div>
  </button>

  <!-- Pointer-only grips, sitting above the body so events land here first. They
       used to carry role="slider" with tabindex="-1" and no key handler, so they
       announced as sliders that could never be focused; keyboard resize is on the
       card (Alt+Arrow). Width scales with the card so a short one keeps more
       middle to drag than edge to resize. -->
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
    <!-- `group-hover:` (unnamed) never matched the `group/card` root, so these
         grips were invisible until the card was selected. -->
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
