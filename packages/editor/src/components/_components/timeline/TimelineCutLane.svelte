<script lang="ts">
import { Scissors, X } from "@recast/icons";
import { type TimelineCut } from "../../../lib/timeline/cuts";
import { originalToOutput, outputToOriginal } from "../../../lib/timeline/time-map";
import type { EditorStore } from "../../../stores/editor-store.svelte";
import { dragEngaged, PRECISION_SCALE } from "./timeline-card-drag.logic";
import {
	CLIP_BASE,
	CLIP_FOCUS,
	CLIP_HOVER,
	CLIP_SELECTED,
	clipSurface,
} from "./timeline-clip.styles";
import { clampCutMove, clampCutResize } from "./timeline-cutlane.logic";
import { frameStep } from "./timeline-helpers";
import {
	CUT_LANE_HEIGHT_PX,
	cardSpan,
	edgeHandleWidth,
	LANE_PADDING_PX,
	ROW_HEIGHT_PX,
} from "./timeline-stack";

// Hosts cut bands and deliberately draws NO waveform: it used to copy the envelope in exactly when the user hid the Audio lane.

interface Props {
	store: EditorStore;
	pixelsPerSecond: number;
	duration: number;
	fps: number;
}

let { store, pixelsPerSecond, duration, fps }: Props = $props();

// Cuts shorter than this are dropped. A sub-100ms removal reads as a glitch.
const MIN_CUT = 0.1;
// A narrower band can't hold two grips and a middle; kept under the other lanes' 28px so it doesn't overstate the removal.
const MIN_BAND_PX = 16;

let laneEl = $state<HTMLDivElement | null>(null);
const surface = clipSurface("cut");

// An applied cut collapses to a seam; with 'Show cut gaps' the map re-spaces it, so the same band UI renders a gap.
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
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
	startClientX: number;
	/** False until the pointer clears the drag threshold (bands only). */
	engaged: boolean;
	/** Shift held: pointer travel is damped for fine positioning. */
	precision: boolean;
}
let drag = $state<DragState | null>(null);
// Pushed on the first real move: clicking a band to select it used to leave an undo entry that changed nothing.
let dragUndoPushed = false;

// Previewed, then committed on release: applying at MIN_CUT collapsed the map mid-gesture and the band shrank under the cursor.
let pending = $state<{ start: number; end: number } | null>(null);

function timeAt(clientX: number): number {
	if (!laneEl) return 0;
	const x = clientX - laneEl.getBoundingClientRect().left;
	// Pointer is in OUTPUT pixels → output seconds → original time.
	return Math.min(duration, Math.max(0, outputToOriginal(store.renderMap, x / pixelsPerSecond)));
}

function onLaneDown(e: PointerEvent) {
	// Only the bare lane background starts a create-drag, and left button only: a right-drag is the context menu.
	if (e.target !== laneEl || duration <= 0 || e.button !== 0) return;
	// The razor owns clicks timeline-wide, so let this one bubble to the scroller's razor handler.
	if (store.timelineTool === "razor") return;
	// Bypassed track: refuse the edit rather than carve a cut that silently wouldn't apply.
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
		startClientX: e.clientX,
		engaged: true,
		precision: e.shiftKey,
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
	dragUndoPushed = false;
	drag = {
		mode,
		pointerId: e.pointerId,
		id: cut.id,
		anchorTime: timeAt(e.clientX),
		originStart: cut.start,
		originEnd: cut.end,
		startClientX: e.clientX,
		engaged: false,
		precision: e.shiftKey,
	};
	laneEl.setPointerCapture(e.pointerId);
}

function onMove(e: PointerEvent) {
	if (!drag || e.pointerId !== drag.pointerId) return;
	// A band press stays a click until it clears the threshold, so selecting can't nudge it or leave a no-op undo entry.
	if (drag.mode !== "create" && !drag.engaged) {
		if (!dragEngaged(e.clientX, drag.startClientX)) return;
		drag.engaged = true;
	}
	// Shift damps travel; re-seed the anchor on a modifier flip so the change in gearing is continuous, not a jump.
	if (e.shiftKey !== drag.precision) {
		drag.precision = e.shiftKey;
		drag.anchorTime = timeAt(e.clientX);
		const live = drag.id ? store.cuts.find((c) => c.id === drag!.id) : null;
		if (live) {
			drag.originStart = live.start;
			drag.originEnd = live.end;
		}
	}
	const raw = timeAt(e.clientX);
	const t = drag.precision ? drag.anchorTime + (raw - drag.anchorTime) * PRECISION_SCALE : raw;

	if (drag.mode === "create") {
		const lo = Math.min(drag.anchorTime, t);
		const hi = Math.max(drag.anchorTime, t);
		// Preview only. The map stays put, so `timeAt` keeps tracking the cursor.
		pending = hi - lo >= MIN_CUT ? { start: lo, end: hi } : null;
		return;
	}

	if (!drag.id) return;
	// A drag is one discrete action → one undo entry.
	if (!dragUndoPushed) {
		store.pushUndoState();
		dragUndoPushed = true;
	}
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
	// addCut() pushes undo itself; pushing here too left a duplicate snapshot that made the second Ctrl+Z look broken.
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

// Mirrors ZoomLayerCard: Shift is 1s, plain is one frame, Alt resizes the trailing edge.
function onBandKeydown(e: KeyboardEvent, cut: TimelineCut) {
	if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
	if (duration <= 0) return;
	e.preventDefault();
	e.stopPropagation();

	store.pushUndoStateCoalesced(`nudge-cut-${cut.id}`, 600);
	const delta = (e.key === "ArrowLeft" ? -1 : 1) * (e.shiftKey ? 1 : frameStep(fps));
	const next = e.altKey
		? clampCutResize({
				edge: "r",
				originStart: cut.start,
				originEnd: cut.end,
				delta,
				duration,
				minCut: MIN_CUT,
			})
		: clampCutMove({ originStart: cut.start, originEnd: cut.end, delta, duration });
	store.updateCut(cut.id, next.start, next.end);
}
</script>

<div
  bind:this={laneEl}
  role="presentation"
  onpointerdown={onLaneDown}
  onpointermove={onMove}
  onpointerup={onUp}
  onpointercancel={onUp}
  class="relative mt-1.5 cursor-crosshair rounded-md bg-muted/20 px-1.5 py-1.5"
  style="min-height: {CUT_LANE_HEIGHT_PX}px;"
>
  <!-- Live preview of the span the release will remove. -->
  {#if pending}
    {@const px = xOf(pending.start)}
    {@const pw = Math.max(2, xOf(pending.end) - px)}
    <!-- Same body as a committed band (at 80%), so the preview shows what the
         release will actually leave behind. -->
    <div
      class="pointer-events-none absolute z-10 rounded-[4px] opacity-80 {surface.fill}"
      style="left: {px}px; width: {pw}px; top: {LANE_PADDING_PX}px; height: {ROW_HEIGHT_PX}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-on) 18%, transparent) 5px, color-mix(in srgb, var(--lane-on) 18%, transparent) 10px);"
    >
      {#if pw > 44}
        <span
          class="pointer-events-none absolute inset-0 flex items-center justify-center font-mono text-[9px] font-bold text-lane-on"
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
      class="pointer-events-none flex h-8 items-center justify-center text-[11px] text-muted-foreground"
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
        class="group/seam absolute z-6 w-3 -translate-x-1/2 cursor-pointer"
        style="left: {cutLeft}px; top: {LANE_PADDING_PX}px; height: {ROW_HEIGHT_PX}px;"
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
      {@const band = cardSpan(cutLeft, cutLeft + cutW, MIN_BAND_PX)}
      {@const w = band.width}
      {@const gripPx = edgeHandleWidth(w)}
      {@const isSel = store.selectedCutId === cut.id}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        role="button"
        tabindex="0"
        data-selectable
        aria-pressed={isSel}
        aria-label={`Removed section, ${(cut.end - cut.start).toFixed(2)} seconds. Arrow keys move it, Alt+Arrow resizes it; press Delete to restore.`}
        onpointerdown={(e) => onBandDown(e, cut, "move")}
        onfocus={() => (store.selectedCutId = cut.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            store.selectedCutId = cut.id;
            return;
          }
          onBandKeydown(e, cut);
        }}
        title="Removed section · {(cut.end - cut.start).toFixed(2)}s"
        class="group/cut absolute cursor-grab active:cursor-grabbing {CLIP_BASE} {CLIP_FOCUS} {surface.fill} {isSel
          ? CLIP_SELECTED
          : CLIP_HOVER}"
        style="left: {band.left}px; width: {w}px; top: {LANE_PADDING_PX}px; height: {ROW_HEIGHT_PX}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-on) 18%, transparent) 5px, color-mix(in srgb, var(--lane-on) 18%, transparent) 10px);"
      >
        <!-- Grips scale with the band so a short one always keeps more middle to
             drag than edge to resize; two fixed 6px grips on a 16px band did not. -->
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-l")}
          class="absolute inset-y-0 left-0 cursor-ew-resize {surface.grip} opacity-0 transition-opacity group-hover/cut:opacity-100"
          style="width: {gripPx}px;"
        ></div>
        <div
          role="presentation"
          onpointerdown={(e) => onBandDown(e, cut, "resize-r")}
          class="absolute inset-y-0 right-0 cursor-ew-resize {surface.grip} opacity-0 transition-opacity group-hover/cut:opacity-100"
          style="width: {gripPx}px;"
        ></div>

        {#if w > 44}
          <span
            class="pointer-events-none absolute inset-0 flex items-center justify-center font-mono text-[9px] font-bold text-lane-on"
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
