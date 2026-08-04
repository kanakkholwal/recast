/** Block height in px. Must match the height the card renders at. */
export const ROW_HEIGHT_PX = 36;
export const ZOOM_ROW_HEIGHT_PX = ROW_HEIGHT_PX;
export const CLIP_ROW_HEIGHT_PX = ROW_HEIGHT_PX;
/** Vertical space between stacked rows. */
export const ROW_SPACING_PX = 4;
/** Horizontal breathing room required between two cards sharing a row. */
export const ROW_GAP_PX = 4;
/** Narrowest a card may render, so a one-frame item stays clickable. */
export const CARD_MIN_WIDTH_PX = 36;
// Fixed-height lanes. These live here, not in the components, because the track
// RAIL sizes its label rows from the same numbers — when the clip bar owned its
// own `h-12` the rail had to hard-code a matching class, and the two drifted.
/** Lane padding above and below the stack, matching the lane's py class. */
export const LANE_PADDING_PX = 6;

/** Clip bar. Taller than a lane block on purpose — it's the spine, and it
 *  carries thumbnails plus a name bar. Also the box a storyboard cell
 *  cover-crops into. */
export const CLIP_LANE_HEIGHT_PX = 56;
/** Lanes holding a single full-width block. Derived, so their block lands at
 *  exactly ROW_HEIGHT_PX like every stacked one. */
export const AUDIO_LANE_HEIGHT_PX = ROW_HEIGHT_PX + LANE_PADDING_PX * 2;
export const CUT_LANE_HEIGHT_PX = AUDIO_LANE_HEIGHT_PX;
/** Lane border width. Counted because a card is positioned in the lane's
 *  PADDING box, which excludes the border but includes the padding. Lanes are
 *  flush tracks (no outline — the rail carries lane identity), so this is 0;
 *  kept as a named term so the height maths still says what it accounts for. */
export const LANE_BORDER_PX = 0;

export interface CardSpan {
	id: string;
	left: number;
	right: number;
}

/**
 * Row index for each span such that no two cards sharing a row come within
 * `ROW_GAP_PX` of each other. Rows fill left-to-right (so the packing doesn't
 * depend on the store's ordering), and the result is returned in INPUT order.
 *
 * `pinned` holds ids to a row they must keep. A card being dragged is pinned
 * for the length of the gesture: re-packing on every pointer move used to
 * teleport it to another row the moment it touched a neighbour, so the card
 * left the cursor mid-drag. Everything else still flows around it.
 */
export function packRows(spans: CardSpan[], pinned?: ReadonlyMap<string, number>): number[] {
	const rows = new Array<number>(spans.length).fill(0);
	// Occupied intervals per row, not just each row's rightmost edge: a pinned
	// card can be placed out of left-to-right order, and a later card must be
	// able to take the room before it rather than being pushed down a row.
	const occupied: Array<Array<{ left: number; right: number }>> = [];

	function place(index: number, row: number) {
		while (occupied.length <= row) occupied.push([]);
		occupied[row].push({ left: spans[index].left, right: spans[index].right });
		rows[index] = row;
	}
	function fits(row: number, s: CardSpan): boolean {
		return (occupied[row] ?? []).every(
			(o) => s.left >= o.right + ROW_GAP_PX || s.right + ROW_GAP_PX <= o.left,
		);
	}

	const byLeft = spans.map((_, i) => i).sort((a, b) => spans[a].left - spans[b].left);
	const free: number[] = [];
	for (const i of byLeft) {
		const row = pinned?.get(spans[i].id);
		if (row === undefined) free.push(i);
		else place(i, Math.max(0, row));
	}
	for (const i of free) {
		let row = occupied.findIndex((_, r) => fits(r, spans[i]));
		if (row === -1) row = occupied.length;
		place(i, row);
	}
	return rows;
}

/** Lane height needed to show `rows` stacked cards without clipping. */
export function laneHeight(rows: number, rowHeightPx = ROW_HEIGHT_PX): number {
	const n = Math.max(1, rows);
	const stack = n * rowHeightPx + (n - 1) * ROW_SPACING_PX;
	return stack + LANE_PADDING_PX * 2 + LANE_BORDER_PX * 2;
}

/** Top offset of a card on the given row, relative to the lane's padding box. */
export function rowTop(row: number, rowHeightPx = ROW_HEIGHT_PX): number {
	return LANE_PADDING_PX + row * (rowHeightPx + ROW_SPACING_PX);
}

/**
 * On-screen span of a card. A sliver is widened around its own centre rather
 * than only to the right, so the card stays over the time it represents and its
 * end handle doesn't sit far past the real end time.
 */
export function cardSpan(
	leftPx: number,
	rightPx: number,
	minWidthPx = CARD_MIN_WIDTH_PX,
): { left: number; width: number } {
	const width = rightPx - leftPx;
	if (width >= minWidthPx) return { left: leftPx, width };
	const centre = leftPx + width / 2;
	return {
		left: Math.max(0, centre - minWidthPx / 2),
		width: minWidthPx,
	};
}

/**
 * Width of each edge-resize target. Scales with the card so a short card always
 * keeps more middle to drag than edge to resize; two fixed 8px handles on a
 * 28px card left almost nothing to grab for moving.
 *
 * Floored at 5px: `Math.max(1, …)` gave a sliver card a 1px target that was
 * effectively impossible to hit.
 */
export function edgeHandleWidth(cardWidthPx: number): number {
	const byShare = Math.round(cardWidthPx / 3.2);
	const capped = Math.min(12, Math.max(5, byShare));
	// Never let the two grips eat the whole card; the middle has to stay draggable.
	return Math.max(1, Math.min(capped, Math.floor(cardWidthPx * 0.4)));
}

/**
 * How far a resize grip reaches OUTSIDE its card. An edge target that stops
 * exactly at the border means aiming at the last pixel; a small overhang makes
 * the boundary itself grabbable. Kept under `ROW_GAP_PX` so two cards sharing a
 * row can't have overlapping grips.
 */
export const EDGE_HIT_OVERHANG_PX = 3;

export interface PlacedCard {
	id: string;
	left: number;
	width: number;
	top: number;
	row: number;
}

export interface LaneCardLayout {
	cards: PlacedCard[];
	rowCount: number;
	height: number;
}

/**
 * Full layout for one lane: every card placed, and the lane height that fits
 * them. Computed once by the timeline so the track rail and the lane body can
 * never disagree about how tall a lane is -- the rail used to hard-code each
 * lane's height as a Tailwind class, which silently broke the moment a lane
 * could grow.
 *
 * `xOf` maps an original time to a pixel offset, keeping this free of the
 * store and the time map.
 */
export function cardLayout(
	items: readonly { id: string; start: number; end: number }[],
	xOf: (t: number) => number,
	opts: {
		minWidthPx?: number;
		rowHeightPx?: number;
		/** Rows to hold fixed, e.g. the card under an active drag. */
		pinnedRows?: ReadonlyMap<string, number>;
	} = {},
): LaneCardLayout {
	const minWidth = opts.minWidthPx ?? CARD_MIN_WIDTH_PX;
	const rowHeight = opts.rowHeightPx ?? ROW_HEIGHT_PX;
	const spans = items.map((it) => {
		const s = cardSpan(xOf(it.start), xOf(it.end), minWidth);
		return { id: it.id, left: s.left, right: s.left + s.width, width: s.width };
	});
	const rows = packRows(spans, opts.pinnedRows);
	const rowCount = rows.length ? Math.max(...rows) + 1 : 0;
	return {
		cards: spans.map((s, i) => ({
			id: s.id,
			left: s.left,
			width: s.width,
			top: rowTop(rows[i], rowHeight),
			row: rows[i],
		})),
		rowCount,
		height: laneHeight(rowCount, rowHeight),
	};
}
