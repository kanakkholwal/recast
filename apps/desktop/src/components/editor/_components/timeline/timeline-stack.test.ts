import { describe, expect, it } from "vitest";
import {
	cardSpan,
	edgeHandleWidth,
	laneHeight,
	packRows,
	ROW_GAP_PX,
	rowTop,
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
		expect(rowTop(0, 30)).toBe(0);
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

	it("never returns a target too small to hit", () => {
		expect(edgeHandleWidth(28)).toBeGreaterThanOrEqual(4);
		expect(edgeHandleWidth(1)).toBeGreaterThanOrEqual(1);
	});
});
