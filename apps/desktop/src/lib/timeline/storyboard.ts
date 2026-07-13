/**
 * Pure geometry for the YouTube-style hover storyboard sprite: the grid sizing
 * (how many cells, the column/row layout) and the time↔cell↔crop mapping. Shared
 * by the decode worker (filmstrip-worker.ts), which packs frames into the sheet,
 * and the hover preview (Timeline.svelte), which crops one cell as a CSS
 * background. Kept here so both sides agree and the math stays unit-tested.
 */

/** Cells per second of source, before clamping. ~1/sec reads well and keeps the
 *  decode bounded (it's really bounded by GOP count, not cell count). */
const MIN_CELLS = 40;
const MAX_CELLS = 200;
const MAX_COLS = 10;

/** Grid sizing derived from the clip duration. */
export interface StoryboardPlan {
	count: number;
	cols: number;
	rows: number;
}

export function planStoryboard(durationSec: number): StoryboardPlan {
	const count = Math.min(
		MAX_CELLS,
		Math.max(MIN_CELLS, Math.ceil(Math.max(0, durationSec))),
	);
	const cols = Math.min(MAX_COLS, count);
	const rows = Math.ceil(count / cols);
	return { count, cols, rows };
}

/** Original-time sampled by cell `cell`, its centre, so the frame represents
 *  the middle of the cell's slice rather than its leading edge. */
export function storyboardSampleSec(
	cell: number,
	count: number,
	durationSec: number,
): number {
	if (count <= 0) return 0;
	return ((cell + 0.5) / count) * durationSec;
}

/** Cell index covering `originalSec`, clamped to [0, count-1]. */
export function storyboardCellIndex(
	originalSec: number,
	count: number,
	durationSec: number,
): number {
	if (count <= 0 || durationSec <= 0) return 0;
	const i = Math.floor((originalSec / durationSec) * count);
	return Math.min(count - 1, Math.max(0, i));
}

/** The built sprite's grid + cell pixel size (no URL; that's the caller's). */
export interface StoryboardMeta {
	cols: number;
	rows: number;
	cellW: number;
	cellH: number;
	count: number;
	durationSec: number;
}

/** CSS background geometry to crop the cell covering `originalSec`, scaled so a
 *  cell renders `displayH` tall (aspect preserved). */
export interface StoryboardCrop {
	dispW: number;
	bgW: number;
	bgH: number;
	offX: number;
	offY: number;
}

export function storyboardCrop(
	meta: StoryboardMeta,
	originalSec: number,
	displayH: number,
): StoryboardCrop {
	const i = storyboardCellIndex(originalSec, meta.count, meta.durationSec);
	const dispW = meta.cellH > 0 ? (meta.cellW / meta.cellH) * displayH : displayH;
	return {
		dispW,
		bgW: meta.cols * dispW,
		bgH: meta.rows * displayH,
		offX: (i % meta.cols) * dispW,
		offY: Math.floor(i / meta.cols) * displayH,
	};
}

/** Seconds of source covered by one cell. The strip only needs its own decoded
 *  frames when it is finer-grained than this. */
export function storyboardCellSec(meta: StoryboardMeta): number {
	if (meta.count <= 0) return Number.POSITIVE_INFINITY;
	return meta.durationSec / meta.count;
}

/** CSS background geometry to fill an arbitrary `boxW`×`boxH` tile with the cell
 *  covering `originalSec`, scaled to COVER and centre-cropped (object-fit:cover
 *  for a sprite). Filmstrip tiles are variable width, so `storyboardCrop`'s
 *  exact-width box does not fit them. */
export function storyboardCoverCrop(
	meta: StoryboardMeta,
	originalSec: number,
	boxW: number,
	boxH: number,
): { bgW: number; bgH: number; offX: number; offY: number } {
	const i = storyboardCellIndex(originalSec, meta.count, meta.durationSec);
	if (meta.cellW <= 0 || meta.cellH <= 0) {
		return { bgW: 0, bgH: 0, offX: 0, offY: 0 };
	}
	const scale = Math.max(boxW / meta.cellW, boxH / meta.cellH);
	const cw = meta.cellW * scale;
	const ch = meta.cellH * scale;
	return {
		bgW: meta.cols * cw,
		bgH: meta.rows * ch,
		// Centre-crop the overflow so the frame stays centred in the tile.
		offX: (i % meta.cols) * cw + (cw - boxW) / 2,
		offY: Math.floor(i / meta.cols) * ch + (ch - boxH) / 2,
	};
}
