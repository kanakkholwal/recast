import { describe, expect, it } from "vitest";
import {
	AUDIO_LANE_HEIGHT_PX,
	cardLayout,
	cardSpan,
	CLIP_LANE_HEIGHT_PX,
	CLIP_ROW_HEIGHT_PX,
	CUT_LANE_HEIGHT_PX,
	edgeHandleWidth,
	LANE_BORDER_PX,
	LANE_PADDING_PX,
	laneHeight,
	packRows,
	ROW_GAP_PX,
	ROW_HEIGHT_PX,
	rowTop,
	ZOOM_ROW_HEIGHT_PX,
} from "./timeline-stack";

function span(id: string, left: number, right: number) {
	return { id, left, right };
}

describe("packRows", () => {
	it("keeps everything on one row when nothing overlaps", () => {
		const rows = packRows([span("a", 0, 40), span("b", 60, 100), span("c", 140, 200)]);
		expect(rows).toEqual([0, 0, 0]);
	});

	// The bug this exists for: two cards at the same moment used to render at
	// top: 50% on top of each other, and the covered one could not be clicked.
	it("pushes an overlapping card onto the next row", () => {
		const rows = packRows([span("a", 0, 100), span("b", 50, 150)]);
		expect(rows).toEqual([0, 1]);
	});

	it("reuses the first row that has room", () => {
		const rows = packRows([
			span("a", 0, 100),
			span("b", 50, 150),
			// Starts clear of "a" but still overlaps "b", so row 0 is free again.
			span("c", 110, 200),
		]);
		expect(rows).toEqual([0, 1, 0]);
	});

	it("stacks a full pile one row per card", () => {
		const rows = packRows([span("a", 0, 100), span("b", 10, 110), span("c", 20, 120)]);
		expect(rows).toEqual([0, 1, 2]);
	});

	// Rows fill left-to-right so the packing can't depend on the store's ordering,
	// but the result comes back aligned to the input array.
	it("packs left-to-right and returns rows in input order", () => {
		expect(packRows([span("late", 200, 300), span("early", 0, 100)])).toEqual([0, 0]);
		expect(packRows([span("late", 50, 300), span("early", 0, 100)])).toEqual([1, 0]);
	});

	it("treats the gap as occupied so neighbours never touch", () => {
		expect(packRows([span("a", 0, 100), span("b", 100 + ROW_GAP_PX - 1, 200)])).toEqual([0, 1]);
		expect(packRows([span("a", 0, 100), span("b", 100 + ROW_GAP_PX, 200)])).toEqual([0, 0]);
	});

	it("returns nothing for an empty lane", () => {
		expect(packRows([])).toEqual([]);
	});
});

// The bug this exists for: re-packing on every pointer move moved the DRAGGED
// card to another row the instant it touched a neighbour, so it left the cursor.
describe("packRows with a pinned card", () => {
	const pin = (id: string, row: number) => new Map([[id, row]]);

	it("keeps the pinned card on its row and moves the other one instead", () => {
		const spans = [span("a", 0, 100), span("dragged", 50, 150)];
		expect(packRows(spans)).toEqual([0, 1]);
		expect(packRows(spans, pin("dragged", 0))).toEqual([1, 0]);
	});

	it("holds the row even once the overlap clears", () => {
		const spans = [span("a", 0, 100), span("dragged", 300, 400)];
		expect(packRows(spans, pin("dragged", 1))).toEqual([0, 1]);
	});

	// A pin placed out of left-to-right order must not push earlier cards down:
	// tracking only each row's rightmost edge did exactly that.
	it("lets an earlier card take the room before a pinned one", () => {
		const spans = [span("early", 0, 50), span("dragged", 300, 400)];
		expect(packRows(spans, pin("dragged", 0))).toEqual([0, 0]);
	});

	it("ignores a pin for an id that isn't in the lane", () => {
		const spans = [span("a", 0, 100), span("b", 50, 150)];
		expect(packRows(spans, pin("gone", 0))).toEqual([0, 1]);
	});
});

// One block height across every lane, so rows line up into a grid. The
// single-block lanes (audio, cuts) derive their height from it rather than
// carrying their own number, which is how they drifted apart before.
describe("block height is one number", () => {
	it("is 36px, and every lane row height agrees", () => {
		expect(ROW_HEIGHT_PX).toBe(36);
		expect(ZOOM_ROW_HEIGHT_PX).toBe(ROW_HEIGHT_PX);
		expect(CLIP_ROW_HEIGHT_PX).toBe(ROW_HEIGHT_PX);
	});

	it("leaves exactly one block inside a single-block lane", () => {
		for (const lane of [AUDIO_LANE_HEIGHT_PX, CUT_LANE_HEIGHT_PX]) {
			expect(lane - LANE_PADDING_PX * 2).toBe(ROW_HEIGHT_PX);
		}
	});

	// The spine carries thumbnails and a name bar, so it is deliberately taller.
	it("keeps the clip bar taller than a lane block", () => {
		expect(CLIP_LANE_HEIGHT_PX).toBeGreaterThan(ROW_HEIGHT_PX);
	});

	it("stacks one block plus padding for a one-row lane", () => {
		expect(laneHeight(1)).toBe(ROW_HEIGHT_PX + LANE_PADDING_PX * 2 + LANE_BORDER_PX * 2);
	});
});

describe("laneHeight", () => {
	it("reserves the minimum for an empty lane", () => {
		expect(laneHeight(0)).toBeGreaterThan(0);
	});

	it("grows by one row's worth per extra row", () => {
		const one = laneHeight(1);
		const two = laneHeight(2);
		const three = laneHeight(3);
		expect(two - one).toBe(three - two);
		expect(two).toBeGreaterThan(one);
	});

	// The zoom lane's cards are taller than the annotation lane's, so both the
	// height and the row offsets have to follow the caller's row height.
	it("follows a taller row height", () => {
		expect(laneHeight(2, 30)).toBeGreaterThan(laneHeight(2, 26));
		expect(rowTop(1, 30)).toBeGreaterThan(rowTop(1, 26));
	});

	// A lane's absolute children are positioned in its PADDING box, so a row at
	// top 0 sits against the padding edge and every pixel of slack pools at the
	// bottom -- a single-row lane looked top-heavy by the full padding.
	it("centres a single row between the lane's padding edges", () => {
		for (const rowHeight of [22, 26, 30]) {
			const paddingBox = laneHeight(1, rowHeight) - LANE_BORDER_PX * 2;
			const top = rowTop(0, rowHeight);
			expect(top, `row height ${rowHeight}`).toBe(paddingBox - top - rowHeight);
		}
	});
});

describe("cardSpan", () => {
	it("passes a comfortable card straight through", () => {
		expect(cardSpan(100, 260)).toEqual({ left: 100, width: 160 });
	});

	// A one-frame annotation is a sub-pixel sliver. It gets widened to stay
	// grabbable, centred on its real span so the card doesn't drift off its time.
	it("widens a sliver around its own centre rather than only rightward", () => {
		const s = cardSpan(100, 104);
		expect(s.width).toBeGreaterThanOrEqual(28);
		expect(s.left + s.width / 2).toBeCloseTo(102, 6);
	});

	it("never starts a widened card at a negative offset", () => {
		const s = cardSpan(0, 2);
		expect(s.left).toBe(0);
	});
});

describe("edgeHandleWidth", () => {
	it("is a comfortable target on a wide card", () => {
		expect(edgeHandleWidth(400)).toBeGreaterThanOrEqual(10);
	});

	// Two fixed 8px handles on a 28px card left 12px to grab for moving, so a
	// short card was almost impossible to drag without resizing it instead.
	it("always leaves more card to move than to resize", () => {
		for (const w of [28, 32, 40, 60, 120, 400]) {
			const handle = edgeHandleWidth(w);
			expect(w - handle * 2, `width ${w}`).toBeGreaterThan(handle);
		}
	});

	// `Math.max(1, …)` used to hand a sliver card a 1px target, which no pointer
	// can reliably hit.
	it("stays hittable on a narrow card", () => {
		for (const w of [16, 20, 28]) {
			expect(edgeHandleWidth(w), `width ${w}`).toBeGreaterThanOrEqual(5);
		}
	});

	it("never returns a target too small to hit", () => {
		expect(edgeHandleWidth(28)).toBeGreaterThanOrEqual(4);
		expect(edgeHandleWidth(1)).toBeGreaterThanOrEqual(1);
	});
});

describe("cardLayout", () => {
	// 10px per second, so times read straight off as tens of pixels.
	const xOf = (t: number) => t * 10;

	it("is empty but still claims a lane height", () => {
		const l = cardLayout([], xOf);
		expect(l.cards).toEqual([]);
		expect(l.rowCount).toBe(0);
		expect(l.height).toBe(laneHeight(0));
	});

	it("places non-overlapping cards on one row", () => {
		const l = cardLayout(
			[
				{ id: "a", start: 0, end: 5 },
				{ id: "b", start: 10, end: 15 },
			],
			xOf,
		);
		expect(l.rowCount).toBe(1);
		expect(l.cards.map((c) => c.top)).toEqual([rowTop(0), rowTop(0)]);
		expect(l.cards[0]).toMatchObject({ left: 0, width: 50 });
	});

	it("stacks an overlap onto a second row and grows the lane", () => {
		const flat = cardLayout([{ id: "a", start: 0, end: 10 }], xOf);
		const stacked = cardLayout(
			[
				{ id: "a", start: 0, end: 10 },
				{ id: "b", start: 5, end: 15 },
			],
			xOf,
		);
		expect(stacked.rowCount).toBe(2);
		expect(stacked.cards[1].top).toBeGreaterThan(0);
		expect(stacked.height).toBeGreaterThan(flat.height);
	});

	// The rail and the lane body both read `height`, so it must always be the
	// height that actually fits `rowCount` rows at the given row height.
	it("reports a height that matches its own row count and row height", () => {
		const l = cardLayout(
			[
				{ id: "a", start: 0, end: 10 },
				{ id: "b", start: 5, end: 15 },
				{ id: "c", start: 6, end: 16 },
			],
			xOf,
			{ rowHeightPx: 30 },
		);
		expect(l.rowCount).toBe(3);
		expect(l.height).toBe(laneHeight(3, 30));
		expect(l.cards[2].top).toBe(rowTop(2, 30));
	});

	it("honours a lane's own minimum card width", () => {
		const l = cardLayout([{ id: "a", start: 1, end: 1.05 }], xOf, { minWidthPx: 32 });
		expect(l.cards[0].width).toBe(32);
	});
});
