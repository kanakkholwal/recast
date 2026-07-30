<script lang="ts">
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
import { type FilmstripTile, planFilmstrip } from "$lib/timeline/filmstrip";
import type { TileProvider } from "$lib/timeline/filmstrip-source";
import { storyboardCellSec, storyboardCoverCrop } from "$lib/timeline/storyboard";
import { deriveSeams } from "$lib/timeline/segments";
import { motionDuration } from "$lib/motion.svelte";
import { Gauge, RotateCcw, SquareSplitHorizontal, Trash2 } from "@recast/icons";
import * as ContextMenu from "@recast/ui/context-menu";
import { fade } from "svelte/transition";
import {
	formatTimeByMode,
	formatSmpte,
	frameStep,
	minClipDuration,
	type TimeMode,
} from "./timeline-helpers";
import {
	clampTrimIn,
	clampTrimOut,
	layoutClipBlocks,
	nudgeTrimIn,
	nudgeTrimOut,
} from "./timeline-clipbar.logic";
import {
	applySpineHandle,
	buildSpineHandles,
	canSlip,
	planSlip,
	type SpineHandle,
	type SpineShape,
} from "./timeline-spine.logic";
import { dragEngaged, PRECISION_SCALE } from "./timeline-card-drag.logic";
import { buildSnapTargets, snapTime } from "./timeline-snap";

// Clip bar with thumbnails and in/out trim handles. Owns its drag state;
// the parent only supplies `clientXToOutput` (handles scroll offset) to resolve pointer X.

interface Props {
	store: EditorStore;
	videoEl: HTMLVideoElement | null;
	fps: number;
	duration: number;
	pixelsPerSecond: number;
	clipLeft: number;
	clipWidth: number;
	thumbnailWidth: number;
	timeMode: TimeMode;
	/** Pointer clientX → output seconds (pre-map); trim maps it via a frozen map. */
	clientXToOutput: (clientX: number) => number;
	/** Fired when a slip engages, so the scroller drops the scrub it started
	 *  (block bodies let pointerdown bubble so a click still seeks). */
	onSpineGesture: () => void;
	// Density-based filmstrip. When null, the stretched Rust strip is rendered.
	tileProvider: TileProvider | null;
	filmstripVersion: number;
	viewportLeftPx: number;
	viewportWidthPx: number;
}

let {
	store,
	videoEl,
	fps,
	duration,
	pixelsPerSecond,
	clipLeft,
	clipWidth,
	thumbnailWidth,
	timeMode,
	clientXToOutput,
	onSpineGesture,
	tileProvider,
	filmstripVersion,
	viewportLeftPx,
	viewportWidthPx,
}: Props = $props();

// Target tile width and the cache-key height namespace; overscan decodes a bit
// beyond the viewport so tiles are ready just before they scroll in.
const TILE_TARGET_W = 96;
const TILE_KEY_HEIGHT = 48;
const FILMSTRIP_OVERSCAN = 240;
/** Rendered height of the clip bar (`h-12`), the box a sprite cell fills. */
const CLIP_H = 48;

const formatSpeed = (s: number) => `${s}×`;

// One block per kept segment on the OUTPUT (post-cut) axis: a cut occupies zero
// width and later clips slide left to close the gap. `xOf` maps original time onto that axis.
const pps = $derived(pixelsPerSecond);
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pps;
// Thumbnail strip is laid across this; each block is internally cut-free, so it shows its slice via a margin offset.
const clipDuration = $derived(Math.max(0.0001, store.outPoint - store.inPoint));
const stripFullWidth = $derived(clipDuration * pps);
const thumbW = $derived(
	store.thumbnailStrip.length > 0
		? Math.max(2, stripFullWidth / store.thumbnailStrip.length)
		: thumbnailWidth,
);
const clipBlocks = $derived(layoutClipBlocks(store.segments, xOf, pps, store.inPoint));
// Density-based filmstrip tiles, planned per kept block and virtualized to the
// viewport. Empty (fallback to the stretched strip) when there's no provider.
const filmstripTiles = $derived(
	tileProvider
		? planFilmstrip(
				clipBlocks.map((b) => ({
					key: b.key,
					leftPx: b.left,
					widthPx: b.width,
					originalStart: b.start,
					originalEnd: b.end,
				})),
				{ leftPx: viewportLeftPx, widthPx: viewportWidthPx },
				{
					tileWidthPx: TILE_TARGET_W,
					tileHeightPx: TILE_KEY_HEIGHT,
					overscanPx: FILMSTRIP_OVERSCAN,
				},
			)
		: [],
);
// The clip bar's base layer is the STORYBOARD SPRITE: one image, built once in
// the worker, cropped per tile with background-position. No per-tile decode, no
// cache to evict, and every tile has pixels the moment the sheet lands.
//
// Per-tile decodes are now only a REFINEMENT, requested when the strip is
// finer-grained than the sheet (i.e. zoomed in far enough that adjacent tiles
// would otherwise crop the same cell). They fade in over the sprite, so the
// strip is never blank and never stalls on a decoder.
const storyboard = $derived.by(() => {
	void filmstripVersion;
	return tileProvider?.storyboard();
});
const cellSec = $derived(storyboard ? storyboardCellSec(storyboard) : Number.POSITIVE_INFINITY);
/** Seconds of source one tile spans at the current zoom. */
const tileSpanSec = $derived(pps > 0 ? TILE_TARGET_W / pps : Number.POSITIVE_INFINITY);
// 0.75 leaves a little hysteresis so a nudge of the zoom doesn't flap the decoder on.
const needsSharpTiles = $derived(!storyboard || tileSpanSec < cellSec * 0.75);

const tilesByBlock = $derived.by(() => {
	const map = new Map<number, FilmstripTile[]>();
	for (const tile of filmstripTiles) {
		const list = map.get(tile.blockKey);
		if (list) list.push(tile);
		else map.set(tile.blockKey, [tile]);
	}
	return map;
});
// Tile URLs, re-resolved whenever a freshly decoded tile bumps the version.
const tileUrls = $derived.by(() => {
	void filmstripVersion;
	const map = new Map<string, string | undefined>();
	if (tileProvider) {
		for (const tile of filmstripTiles) {
			map.set(tile.cacheKey, tileProvider.get(tile));
		}
	}
	return map;
});
$effect(() => {
	// Re-runs on `filmstripVersion` too, not just when the plan changes: a tile
	// evicted from the LRU would otherwise stay grey forever, because nothing
	// would ever ask for it again. `request` no-ops on cached/inflight tiles, so
	// this settles immediately once the strip is populated.
	void filmstripVersion;
	if (!needsSharpTiles) return;
	if (tileProvider && filmstripTiles.length > 0) {
		tileProvider.request(filmstripTiles);
	}
});

const splitMarkers = $derived(
	store.splitPoints
		.filter((p) => p > store.inPoint && p < store.outPoint)
		.map((p) => ({ time: p, x: xOf(p) })),
);
// Where a removed cut sits between two kept segments, collapsed to one seam.
// deriveSeams is the pure unit-tested helper; here we only add the output-axis x.
const seamMarkers = $derived(
	deriveSeams(store.segments).map((s) => ({ ...s, x: xOf(s.gapStart) })),
);
function restoreSeam(gapStart: number, gapEnd: number) {
	// Restore every cut that lives inside the collapsed gap.
	for (const c of store.effectiveCuts) {
		if (c.start >= gapStart - 1e-3 && c.end <= gapEnd + 1e-3) {
			store.removeCut(c.id);
		}
	}
}
const inHandleLeft = $derived(clipLeft);
const outHandleLeft = $derived(clipLeft + clipWidth);

// Midpoint is always inside the block, so deleteSegmentAt targets exactly it; park playhead on the join.
function deleteSegment(start: number, end: number) {
	const joinAt = store.deleteSegmentAt((start + end) / 2);
	if (joinAt === null) return;
	store.currentTime = joinAt;
	if (videoEl) videoEl.currentTime = joinAt;
}

// Right-click menu: original time the menu was opened at (set on pointerdown,
// which fires for the right button before `contextmenu`), so "Split here"
// splits exactly where you clicked rather than at the playhead.
const SPEED_PRESETS = [0.5, 1, 1.5, 2] as const;
let menuTime = $state(0);
function rememberMenuTime(clientX: number) {
	menuTime = outputToOriginal(store.renderMap, clientXToOutput(clientX));
}

// Faint audio envelope over the footage, so you can see where to cut. Built in
// output-pixel space (each bucket at `xOf(bucketTime)`) over the kept range
// only; buckets inside a removed cut collapse onto the seam like the cut lane.
let activeTrimHandle = $state<"in" | "out" | null>(null);
// Output-x of the active trim snap target (playhead/region/etc.), or null.
let trimSnapX = $state<number | null>(null);

// `originalAt` = the handle value at pointer-down, for the frames-delta tooltip.
let trimDragContext = $state<{
	which: "in" | "out";
	originalAt: number;
} | null>(null);
function startTrimDrag(event: PointerEvent, which: "in" | "out") {
	if (duration <= 0) return;
	event.preventDefault();
	event.stopPropagation();
	// Single undo entry per drag.
	store.pushUndoState();
	// Un-collapse the axis for the drag: the clip un-brackets to the full
	// recording (trimmed head/tail ghosted), the handle follows the cursor, and
	// dragging outward restores. Reverts to the collapsed view on pointer-up.
	store.isTrimming = true;
	activeTrimHandle = which;
	trimDragContext = {
		which,
		originalAt: which === "in" ? store.inPoint : store.outPoint,
	};
	document.body.style.cursor = "ew-resize";
	(event.currentTarget as Element).setPointerCapture(event.pointerId);
	updateTrimFromPointer(event.clientX, which, true);
	const onMove = (e: PointerEvent) => {
		updateTrimFromPointer(e.clientX, which, true);
	};
	const onUp = (e: PointerEvent) => {
		activeTrimHandle = null;
		trimDragContext = null;
		trimSnapX = null;
		store.isTrimming = false;
		document.body.style.cursor = "";
		try {
			(event.currentTarget as Element).releasePointerCapture(e.pointerId);
		} catch {
			// already released on some browsers
		}
		window.removeEventListener("pointermove", onMove);
		window.removeEventListener("pointerup", onUp);
		window.removeEventListener("pointercancel", onUp);
	};
	window.addEventListener("pointermove", onMove);
	window.addEventListener("pointerup", onUp);
	window.addEventListener("pointercancel", onUp);
}

// Snap the dragged handle to the playhead, clip edges, and region/annotation
// boundaries (not its own point); falls through to the frame grid otherwise.
function snapTrim(raw: number, which: "in" | "out"): number {
	const targets = buildSnapTargets({
		playhead: store.currentTime,
		inPoint: store.inPoint,
		outPoint: store.outPoint,
		duration,
		regions: store.zoomRegions,
		annotations: store.annotations,
	}).filter((target) =>
		which === "in" ? target.kind !== "in-point" : target.kind !== "out-point",
	);
	const tolerance = pps > 0 ? 6 / pps : 0;
	const result = snapTime(raw, targets, tolerance, fps);
	// Surface the active snap as a guide line at the target's position.
	trimSnapX = result.target ? xOf(result.target.time) : null;
	return result.time;
}

function updateTrimFromPointer(clientX: number, which: "in" | "out", scrub = false) {
	// Output px → original time. While trimming, store.renderMap is the full
	// recording axis (stable, not collapsing under the drag), so absolute
	// mapping tracks the cursor and lets the handle move across the whole source.
	const raw = outputToOriginal(store.renderMap, clientXToOutput(clientX));
	const t = snapTrim(raw, which);
	const min = minClipDuration(fps);
	if (which === "in") {
		const next = clampTrimIn(t, store.outPoint, min);
		store.trimStart = next;
		// Park playback at the in point so the preview shows the first kept frame while dragging.
		if (scrub) {
			store.currentTime = next;
			if (videoEl) videoEl.currentTime = next;
		}
	} else {
		const next = clampTrimOut(t, duration, store.inPoint, min);
		store.trimEnd = next;
		if (scrub) {
			// Show the last kept frame (one before the cut), the frame being decided on.
			const previewAt = Math.max(store.inPoint, next - frameStep(fps));
			store.currentTime = previewAt;
			if (videoEl) videoEl.currentTime = previewAt;
		}
	}
}

function nudgeTrimByKey(which: "in" | "out", direction: 1 | -1, second: boolean) {
	if (duration <= 0) return;
	store.pushUndoStateCoalesced(`trim-${which}`, 500);
	const delta = direction * (second ? 1 : frameStep(fps));
	const min = minClipDuration(fps);
	if (which === "in") {
		store.trimStart = nudgeTrimIn(store.inPoint, store.outPoint, delta, min, fps);
	} else {
		store.trimEnd = nudgeTrimOut(store.inPoint, store.outPoint, duration, delta, min, fps);
	}
}

function handleTrimHandleKey(event: KeyboardEvent, which: "in" | "out") {
	if (duration <= 0) return;
	if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
	event.preventDefault();
	event.stopPropagation();
	nudgeTrimByKey(which, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey);
}

// ---- Spine edits (roll / slide / slip) -------------------------------------
//
// All three are length-preserving, so the output axis doesn't move under the
// cursor mid-drag and the pointer can be mapped absolutely, like the trim
// handles do with a frozen map. (Rolling across two segments at DIFFERENT
// speeds does warp the axis; that's rare enough to leave alone.)

/** Shortest removed range, matching TimelineCutLane's MIN_CUT. */
const MIN_CUT = 0.1;

const spineShape = $derived<SpineShape>({
	segments: store.segments,
	cuts: store.effectiveCuts,
	minClip: minClipDuration(fps),
	minCut: MIN_CUT,
});
const spineHandles = $derived(buildSpineHandles(spineShape));

/** `at` is the live boundary time, so a repeated write moves from where the
 *  last one landed rather than from a stale anchor. */
let spineDrag = $state<{
	handle: SpineHandle;
	at: number;
	startClientX: number;
	engaged: boolean;
	precision: boolean;
	anchorTime: number;
	gearOffset: number;
} | null>(null);
let slipDrag = $state<{
	index: number;
	startClientX: number;
	anchorTime: number;
	appliedStart: number;
	engaged: boolean;
	precision: boolean;
	/** Travel banked before the last precision flip, so gearing is continuous. */
	gearOffset: number;
	delta: number;
} | null>(null);
// Deliberately not $state: it's the shape FROZEN at pointer-down, and wrapping
// the store's segment/cut objects in a reactive proxy would be a trap.
let slipShape: SpineShape | null = null;

function originalAtPointer(clientX: number): number {
	return outputToOriginal(store.renderMap, clientXToOutput(clientX));
}

// Same target list the trim handles use, minus the boundary's own position.
// Ctrl/Cmd bypasses magnetism; the frame grid still applies.
function snapBoundary(raw: number, self: number, bypass: boolean): number {
	const targets = bypass
		? []
		: buildSnapTargets({
				playhead: store.currentTime,
				inPoint: store.inPoint,
				outPoint: store.outPoint,
				duration,
				regions: store.zoomRegions,
				annotations: store.annotations,
			}).filter((t) => Math.abs(t.time - self) > 1e-4);
	const tolerance = pps > 0 ? 6 / pps : 0;
	return snapTime(raw, targets, tolerance, fps).time;
}

// A boundary marker is both a drag target and a click target (restore a cut,
// rejoin a split). Set while a drag actually writes, so the click that follows
// pointer-up doesn't also fire the destructive action.
let spineMoved = false;

/** The handle sitting on a marker, or null when the boundary can't move. */
function handleAt(time: number): SpineHandle | null {
	return spineHandles.find((h) => Math.abs(h.at - time) <= 1e-4) ?? null;
}

function startSpineDrag(event: PointerEvent, handle: SpineHandle) {
	if (event.button !== 0 || duration <= 0 || store.timelineTool === "razor") return;
	event.preventDefault();
	event.stopPropagation();
	spineMoved = false;
	spineDrag = {
		handle,
		at: handle.at,
		startClientX: event.clientX,
		engaged: false,
		precision: event.shiftKey,
		anchorTime: originalAtPointer(event.clientX),
		gearOffset: 0,
	};
	document.body.style.cursor = "ew-resize";
	(event.currentTarget as Element).setPointerCapture(event.pointerId);
	const onMove = (e: PointerEvent) => moveSpineDrag(e);
	const onUp = () => {
		spineDrag = null;
		document.body.style.cursor = "";
		window.removeEventListener("pointermove", onMove);
		window.removeEventListener("pointerup", onUp);
		window.removeEventListener("pointercancel", onUp);
	};
	window.addEventListener("pointermove", onMove);
	window.addEventListener("pointerup", onUp);
	window.addEventListener("pointercancel", onUp);
}

function moveSpineDrag(event: PointerEvent) {
	if (!spineDrag) return;
	// A boundary press is a click (restore a cut) until it clears the threshold.
	if (!spineDrag.engaged) {
		if (!dragEngaged(event.clientX, spineDrag.startClientX)) return;
		spineDrag.engaged = true;
	}
	const raw = originalAtPointer(event.clientX);
	// Shift damps travel; re-seeding on the flip keeps the boundary continuous.
	if (event.shiftKey !== spineDrag.precision) {
		const before = gearedBoundary(raw);
		spineDrag.precision = event.shiftKey;
		spineDrag.anchorTime = raw;
		spineDrag.gearOffset = before - raw;
	}
	const geared = gearedBoundary(raw);
	applySpine(spineDrag.handle, snapBoundary(geared, spineDrag.at, event.ctrlKey || event.metaKey));
}

function gearedBoundary(raw: number): number {
	if (!spineDrag) return raw;
	const base = spineDrag.precision
		? spineDrag.anchorTime + (raw - spineDrag.anchorTime) * PRECISION_SCALE
		: raw;
	return base + spineDrag.gearOffset;
}

// One undo entry per gesture, keyed by the boundary so a drag and a run of
// arrow-key nudges each coalesce.
function applySpine(handle: SpineHandle, rawAt: number) {
	store.pushUndoStateCoalesced(`spine-${handle.key}`, 600);
	spineMoved = true;
	const result = applySpineHandle(handle, rawAt, fps);
	if (result.kind === "roll") {
		const from = spineDrag?.at ?? handle.at;
		if (store.moveSplit(from, result.to) && spineDrag) spineDrag.at = result.to;
	} else {
		store.slideCut(result.cutId, result.start, result.end);
		if (spineDrag) spineDrag.at = result.start;
	}
}

function onSpineHandleKey(event: KeyboardEvent, handle: SpineHandle) {
	if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
	event.preventDefault();
	event.stopPropagation();
	const step = (event.shiftKey ? 1 : frameStep(fps)) * (event.key === "ArrowLeft" ? -1 : 1);
	applySpine(handle, handle.at + step);
}

function startSlip(event: PointerEvent, index: number) {
	// Alt-gated: a bare drag across the clip bar is a scrub, and that's the most
	// common gesture in a screen recorder. Alt+drag is the NLE slip modifier.
	//
	// Deliberately does NOT stop propagation: a plain click on a block still has
	// to reach the scroller and seek. The scrub is cancelled below, but only once
	// the pointer has travelled far enough to mean "slip".
	if (!event.altKey || event.button !== 0 || duration <= 0) return;
	if (store.timelineTool === "razor") return;
	if (!canSlip(spineShape, index)) return;
	const seg = spineShape.segments[index];
	slipShape = spineShape;
	slipDrag = {
		index,
		startClientX: event.clientX,
		anchorTime: originalAtPointer(event.clientX),
		appliedStart: seg.start,
		engaged: false,
		precision: event.shiftKey,
		gearOffset: 0,
		delta: 0,
	};
	const onMove = (e: PointerEvent) => moveSlip(e);
	const onUp = () => {
		slipDrag = null;
		slipShape = null;
		document.body.style.cursor = "";
		window.removeEventListener("pointermove", onMove);
		window.removeEventListener("pointerup", onUp);
		window.removeEventListener("pointercancel", onUp);
	};
	window.addEventListener("pointermove", onMove);
	window.addEventListener("pointerup", onUp);
	window.addEventListener("pointercancel", onUp);
}

function moveSlip(event: PointerEvent) {
	if (!slipDrag || !slipShape) return;
	if (!slipDrag.engaged) {
		if (!dragEngaged(event.clientX, slipDrag.startClientX)) return;
		slipDrag.engaged = true;
		document.body.style.cursor = "ew-resize";
		onSpineGesture();
	}
	// Planned from the shape frozen at pointer-down, so every move is absolute
	// and re-applying can't compound. Shift damps the travel; the anchor moves
	// with the modifier so the frames don't jump when it flips.
	const raw = originalAtPointer(event.clientX);
	if (event.shiftKey !== slipDrag.precision) {
		const before = slipDelta(raw);
		slipDrag.precision = event.shiftKey;
		slipDrag.anchorTime = raw;
		slipDrag.gearOffset = before;
	}
	const plan = planSlip(slipShape, slipDrag.index, slipDelta(raw), fps);
	if (!plan) return;
	store.pushUndoStateCoalesced(`slip-${slipDrag.index}`, 600);
	const nextStart = slipShape.segments[slipDrag.index].start + plan.delta;
	store.slipSegment({
		from: slipDrag.appliedStart,
		to: nextStart,
		before: plan.before,
		after: plan.after,
	});
	slipDrag.appliedStart = nextStart;
	slipDrag.delta = plan.delta;
}

/** Travel from the grab point, damped while the precision modifier is held. */
function slipDelta(raw: number): number {
	if (!slipDrag) return 0;
	const travel = raw - slipDrag.anchorTime;
	return slipDrag.gearOffset + (slipDrag.precision ? travel * PRECISION_SCALE : travel);
}

const slipLabel = $derived(
	slipDrag?.engaged
		? `${slipDrag.delta > 0 ? "+" : ""}${Math.round(slipDrag.delta * fps)} f`
		: null,
);
</script>

<div class="relative h-12">
  <!-- Ghost bands: while trimming, the axis un-collapses to the full recording
       and the trimmed head/tail show dimmed, so you can see and re-drag them. -->
  {#if store.isTrimming}
    {#if clipLeft > 1}
      <div
        class="pointer-events-none absolute inset-y-0 left-0 z-8 rounded-l-md border border-border/40 bg-background/60"
        style="width: {clipLeft}px;"
      ></div>
    {/if}
    <div
      class="pointer-events-none absolute inset-y-0 right-0 z-8 rounded-r-md border border-border/40 bg-background/60"
      style="left: {clipLeft + clipWidth}px;"
    ></div>
    {#if trimSnapX !== null}
      <div
        class="pointer-events-none absolute inset-y-0 z-9 w-px bg-primary"
        style="left: {trimSnapX}px;"
      ></div>
    {/if}
  {/if}

  {#each clipBlocks as block, blockIndex (block.key)}
    {@const selected =
      store.selectedClipStart !== null &&
      Math.abs(store.selectedClipStart - block.start) < 1e-4}
    {@const speed = store.segmentSpeedAt(block.start)}
    {@const slippable = canSlip(spineShape, blockIndex)}
    {@const slipping = slipDrag?.engaged && slipDrag.index === blockIndex}
    <ContextMenu.Root>
      <ContextMenu.Trigger>
        {#snippet child({ props })}
          <!-- Select on POINTERDOWN, not click: the timeline scroller calls
               setPointerCapture on its own pointerdown, which redirects pointerup
               and makes the synthesised click land on the scroller (seek) instead
               of here. We don't stop propagation, so the click still seeks too
               (select + seek). Right-click records the time for "Split here". -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            {...props}
            role="button"
            tabindex="-1"
            data-selectable
            title={slippable
              ? "Drag to slip: the frames shift inside the block, the timing doesn't."
              : undefined}
            onpointerdown={(e) => {
              rememberMenuTime(e.clientX);
              // A razor click carves through the clip; it must not also select it.
              if (e.button === 0 && store.timelineTool !== "razor")
                store.selectedClipStart = block.start;
              startSlip(e, blockIndex);
            }}
            class="group/clip absolute inset-y-0 overflow-hidden rounded-md border transition-[box-shadow,border-color] {slippable
              ? 'cursor-ew-resize'
              : 'cursor-pointer'} {selected
              ? 'border-primary ring-2 ring-primary/50'
              : 'border-border/70 hover:border-foreground/30'} {slipping
              ? 'ring-2 ring-primary'
              : ''}"
            style="left: {block.left}px; width: {block.width}px;"
          >
      <!-- Thumbnails are LAYERED, cheapest first, so the strip can never be blank:
             1. the stretched Rust strip (8-12 frames, ready almost immediately),
             2. the storyboard sprite (one image, cropped per tile, no decode),
             3. per-tile decodes, only when zoomed past the sprite's density.
           These used to be if/else branches, so the moment a WebCodecs provider
           existed the Rust strip became unreachable and an undecoded tile showed
           grey instead of the frame it already had. -->
      {#if store.thumbnailStrip.length > 0}
        <div
          class="absolute inset-0 flex"
          style="width: {stripFullWidth}px; margin-left: {block.stripOffset}px;"
        >
          {#each store.thumbnailStrip as frame, index (frame + index)}
            <img
              src={frame}
              alt=""
              class="h-full shrink-0 object-cover"
              style="width: {thumbW}px;"
              draggable="false"
            />
          {/each}
        </div>
      {:else}
        <div class="absolute inset-0 bg-muted/40"></div>
      {/if}

      {#if tileProvider}
        {#each tilesByBlock.get(block.key) ?? [] as tile (tile.cacheKey)}
          {@const url = tileUrls.get(tile.cacheKey)}
          <div
            class="absolute inset-y-0 overflow-hidden"
            style="left: {tile.offsetPx}px; width: {tile.widthPx}px;"
          >
            {#if storyboard}
              {@const c = storyboardCoverCrop(
                storyboard,
                tile.sampleOriginalSec,
                tile.widthPx,
                CLIP_H,
              )}
              <!-- One sprite, cropped by background-position. Costs no decode. -->
              <div
                in:fade={{ duration: motionDuration(140) }}
                class="absolute inset-0"
                style="background-image: url('{storyboard.url}'); background-repeat: no-repeat; background-size: {c.bgW}px {c.bgH}px; background-position: -{c.offX}px -{c.offY}px;"
              ></div>
            {/if}
            {#if url}
              <!-- Sharper per-tile frame, only decoded when zoomed past the
                   sprite's density. Fades in ON TOP, so nothing ever blanks. -->
              <img
                in:fade={{ duration: motionDuration(120) }}
                src={url}
                alt=""
                class="absolute inset-0 h-full w-full object-cover"
                draggable="false"
              />
            {/if}
          </div>
        {/each}
      {/if}

      <!-- Live slip readout: the block holds its place, so the frame offset is
           the only thing that tells you the drag is landing. -->
      {#if slipping && slipLabel}
        <div
          class="pointer-events-none absolute inset-0 z-8 flex items-center justify-center bg-background/35"
        >
          <span
            class="rounded bg-popover px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-foreground shadow-sm"
          >
            Slip {slipLabel}
          </span>
        </div>
      {/if}

      <!-- Read-only speed badge (the editable control lives in the Clip panel). -->
      {#if speed !== 1}
        <div
          title="Clip speed. Edit in the Clip panel."
          class="pointer-events-none absolute left-1 top-1 z-7 flex h-4 items-center gap-0.5 rounded bg-foreground/85 px-1 font-mono text-[9px] font-bold text-background"
        >
          <Gauge class="size-2.5" />
          {formatSpeed(speed)}
        </div>
      {/if}

      <!-- Per-clip ripple delete, only with >1 clip (trim handles remove the whole recording). -->
      {#if clipBlocks.length > 1}
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          type="button"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={() => deleteSegment(block.start, block.end)}
          title="Delete this clip and close the gap"
          class="absolute right-1 top-1 z-7 flex size-4 items-center justify-center rounded bg-background/80 text-muted-foreground opacity-0 backdrop-blur transition-opacity hover:bg-lane-cut hover:text-background group-hover/clip:opacity-100"
        >
          <Trash2 class="size-2.5" />
        </button>
      {/if}
          </div>
        {/snippet}
      </ContextMenu.Trigger>
      <ContextMenu.Content size="sm" class="w-48">
        <ContextMenu.Item onSelect={() => store.splitAt(menuTime)}>
          <SquareSplitHorizontal />
          Split here
        </ContextMenu.Item>
        <ContextMenu.Sub>
          <ContextMenu.SubTrigger>
            <Gauge />
            Speed
          </ContextMenu.SubTrigger>
          <ContextMenu.SubContent>
            <ContextMenu.RadioGroup
              value={String(speed)}
              onValueChange={(v) =>
                store.setSegmentSpeed(block.start, parseFloat(v))}
            >
              {#each SPEED_PRESETS as preset (preset)}
                <ContextMenu.RadioItem value={String(preset)}>
                  {formatSpeed(preset)}
                </ContextMenu.RadioItem>
              {/each}
            </ContextMenu.RadioGroup>
          </ContextMenu.SubContent>
        </ContextMenu.Sub>
        <ContextMenu.Item
          disabled={speed === 1}
          onSelect={() => store.setSegmentSpeed(block.start, 1)}
        >
          <RotateCcw />
          Reset speed
        </ContextMenu.Item>
        {#if clipBlocks.length > 1}
          <ContextMenu.Separator />
          <ContextMenu.Item
            variant="destructive"
            onSelect={() => deleteSegment(block.start, block.end)}
          >
            <Trash2 />
            Delete clip
          </ContextMenu.Item>
        {/if}
      </ContextMenu.Content>
    </ContextMenu.Root>
  {/each}

  <!-- Removed section collapsed to a restorable seam. Click restores it; dragging
       SLIDES the removed window, so both neighbouring blocks change start/end and
       neither the clip nor the removal changes length. -->
  {#each seamMarkers as seam (seam.gapStart)}
    {@const slide = handleAt(seam.gapStart)}
    {@const active = !!slide && spineDrag?.handle.key === slide.key}
    <button
      type="button"
      onpointerdown={(e) => {
        if (slide) startSpineDrag(e, slide);
        else e.stopPropagation();
      }}
      onkeydown={(e) => slide && onSpineHandleKey(e, slide)}
      onclick={() => {
        // A slide ends in a click on this same element; it must not also restore.
        if (spineMoved) {
          spineMoved = false;
          return;
        }
        restoreSeam(seam.gapStart, seam.gapEnd);
      }}
      title={slide
        ? `Removed ${seam.removed.toFixed(2)}s. Drag to slide it, click to restore.`
        : `Removed ${seam.removed.toFixed(2)}s. Click to restore.`}
      class="group/seam absolute inset-y-0 z-9 w-3 -translate-x-1/2 focus-visible:outline-none {slide
        ? 'cursor-ew-resize'
        : 'cursor-pointer'}"
      style="left: {seam.x}px;"
    >
      <div
        class="mx-auto h-full w-0.5 bg-lane-cut/70 transition-all group-hover/seam:w-1 group-hover/seam:bg-lane-cut group-focus-visible/seam:w-1 {active
          ? 'w-1! bg-lane-cut'
          : ''}"
      ></div>
      <span
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 hidden -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm group-hover/seam:block {active
          ? 'block!'
          : ''}"
      >
        {#if active}
          {formatTimeByMode(seam.gapStart, timeMode, fps)} · slide
        {:else}
          −{seam.removed.toFixed(2)}s · {slide ? "drag or restore" : "restore"}
        {/if}
      </span>
    </button>
  {/each}

  <!-- Split between two adjacent clips. Dragging ROLLS the edit (one block grows
       by exactly what the other loses); double-click rejoins. -->
  {#each splitMarkers as marker (marker.time)}
    {@const roll = handleAt(marker.time)}
    {@const active = !!roll && spineDrag?.handle.key === roll.key}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      role="button"
      tabindex={roll ? 0 : -1}
      aria-label="Split at {formatTimeByMode(marker.time, timeMode, fps)}. Drag or press the arrow keys to roll the edit; double-click to rejoin."
      onpointerdown={(e) => {
        if (roll) startSpineDrag(e, roll);
        else e.stopPropagation();
      }}
      onkeydown={(e) => roll && onSpineHandleKey(e, roll)}
      ondblclick={() => store.removeSplit(marker.time)}
      title={roll
        ? "Split. Drag to roll the edit, double-click to rejoin."
        : "Split. Double-click to rejoin."}
      class="group/split absolute inset-y-0 z-9 w-2.5 -translate-x-1/2 focus-visible:outline-none {roll
        ? 'cursor-ew-resize'
        : 'cursor-pointer'}"
      style="left: {marker.x}px;"
    >
      <div
        class="mx-auto h-full w-px bg-lane-markup transition-all group-hover/split:w-0.5 group-focus-visible/split:w-0.5 {active
          ? 'w-0.5!'
          : ''}"
      ></div>
      {#if active}
        <span
          class="pointer-events-none absolute bottom-full left-1/2 mb-1 -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
        >
          {formatTimeByMode(marker.time, timeMode, fps)} · roll
        </span>
      {/if}
    </div>
  {/each}

  <!--
    Trim drag handles, anchored to the in/out points. Each is a narrow vertical
    bar with a larger invisible hit area so grabbing is easy. Pointer events
    stop propagation so we don't fight the timeline's click-to-seek scrub.
  -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="0"
    aria-label="In point"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={store.inPoint}
    aria-valuetext={formatSmpte(store.inPoint, fps)}
    onpointerdown={(e) => startTrimDrag(e, "in")}
    onkeydown={(e) => handleTrimHandleKey(e, "in")}
    class="group absolute inset-y-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
    style="left: {inHandleLeft}px;"
  >
    <div
      class="mx-auto h-full w-1 rounded-l-md bg-primary transition-all group-hover:w-1.5 group-hover:ring-2 group-hover:ring-primary/30"
    ></div>
    {#if activeTrimHandle === "in" && trimDragContext}
      {@const delta = store.inPoint - trimDragContext.originalAt}
      <div
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
      >
        <span>In {formatTimeByMode(store.inPoint, timeMode, fps)}</span>
        {#if delta !== 0}
          <span class="text-muted-foreground"
            >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
          >
        {/if}
      </div>
    {/if}
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="0"
    aria-label="Out point"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={store.outPoint}
    aria-valuetext={formatSmpte(store.outPoint, fps)}
    onpointerdown={(e) => startTrimDrag(e, "out")}
    onkeydown={(e) => handleTrimHandleKey(e, "out")}
    class="group absolute inset-y-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
    style="left: {outHandleLeft}px;"
  >
    <div
      class="mx-auto h-full w-1 rounded-r-md bg-primary transition-all group-hover:w-1.5 group-hover:ring-2 group-hover:ring-primary/30"
    ></div>
    {#if activeTrimHandle === "out" && trimDragContext}
      {@const delta = store.outPoint - trimDragContext.originalAt}
      <div
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
      >
        <span>Out {formatTimeByMode(store.outPoint, timeMode, fps)}</span>
        {#if delta !== 0}
          <span class="text-muted-foreground"
            >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
          >
        {/if}
      </div>
    {/if}
  </div>
</div>
