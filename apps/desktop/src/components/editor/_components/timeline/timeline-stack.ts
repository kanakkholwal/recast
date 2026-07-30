// Vertical stacking for timeline lane cards, plus the two size decisions that
// depend on a card's pixel width.
//
// Cards used to be pinned to `top: 50%`, so two annotations covering the same
// moment drew on top of each other and only the last one in DOM order could be
// clicked or resized. Packing is done in PIXELS, not seconds, because a very
// short card is widened to stay grabbable and can therefore collide with a
// neighbour it doesn't actually overlap in time.

/** Annotation card height in px. Must match the height the card renders at. */
export const ROW_HEIGHT_PX = 26;
/** Zoom cards are taller: they carry a sparkline as well as a label. */
export const ZOOM_ROW_HEIGHT_PX = 30;
/** Audio-clip cards (music, detached voice) sit between the two. */
export const CLIP_ROW_HEIGHT_PX = 22;
/** Vertical space between stacked rows. */
export const ROW_SPACING_PX = 4;
/** Horizontal breathing room required between two cards sharing a row. */
export const ROW_GAP_PX = 4;
/** Narrowest a card may render, so a one-frame item stays clickable. */
export const CARD_MIN_WIDTH_PX = 28;
/** Lane padding above and below the stack, matching the lane's py class. */
export const LANE_PADDING_PX = 6;
/** Lane border width. Counted because a card is positioned in the lane's
 *  PADDING box, which excludes the border but includes the padding. */
export const LANE_BORDER_PX = 1;

export interface CardSpan {
	id: string;
	left: number;
	right: number;
}

/**
 * Row index for each span such that no two cards sharing a row come within
 * `ROW_GAP_PX` of each other. Rows fill left-to-right (so the packing doesn't
 * depend on the store's ordering), and the result is returned in INPUT order.
 */
export function packRows(spans: CardSpan[]): number[] {
	const byLeft = spans.map((_, i) => i).sort((a, b) => spans[a].left - spans[b].left);
	const rowEnds: number[] = [];
	const rows = new Array<number>(spans.length).fill(0);
	for (const i of byLeft) {
		const s = spans[i];
		let row = rowEnds.findIndex((end) => s.left >= end + ROW_GAP_PX);
		if (row === -1) {
			rowEnds.push(s.right);
			row = rowEnds.length - 1;
		} else {
			rowEnds[row] = Math.max(rowEnds[row], s.right);
		}
		rows[i] = row;
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
 */
export function edgeHandleWidth(cardWidthPx: number): number {
	const byShare = Math.floor(cardWidthPx / 3.2);
	return Math.max(1, Math.min(12, byShare));
}

export interface PlacedCard {
	id: string;
	left: number;
	width: number;
	top: number;
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
	opts: { minWidthPx?: number; rowHeightPx?: number } = {},
): LaneCardLayout {
	const minWidth = opts.minWidthPx ?? CARD_MIN_WIDTH_PX;
	const rowHeight = opts.rowHeightPx ?? ROW_HEIGHT_PX;
	const spans = items.map((it) => {
		const s = cardSpan(xOf(it.start), xOf(it.end), minWidth);
		return { id: it.id, left: s.left, right: s.left + s.width, width: s.width };
	});
	const rows = packRows(spans);
	const rowCount = rows.length ? Math.max(...rows) + 1 : 0;
	return {
		cards: spans.map((s, i) => ({
			id: s.id,
			left: s.left,
			width: s.width,
			top: rowTop(rows[i], rowHeight),
		})),
		rowCount,
		height: laneHeight(rowCount, rowHeight),
	};
}
