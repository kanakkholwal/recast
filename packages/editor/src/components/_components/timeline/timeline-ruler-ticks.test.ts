import { describe, expect, it } from "vitest";
import { buildMinorTicks, buildTimeMarkers } from "./timeline-helpers";

const HALF_HOUR = 1800;

describe("ruler tick windowing", () => {
	it("covers the whole duration when no window is given", () => {
		const ticks = buildMinorTicks(10, 60);
		expect(ticks[0]).toBe(0);
		expect(ticks.at(-1)).toBeGreaterThanOrEqual(10);
	});

	it("emits a viewport's worth of ticks instead of the whole timeline", () => {
		// 30 min at 0.25s spacing (pps > 180) is 7,200 ticks unwindowed.
		const all = buildMinorTicks(HALF_HOUR, 200);
		const windowed = buildMinorTicks(HALF_HOUR, 200, { startSec: 600, endSec: 604 });

		expect(all.length).toBeGreaterThan(7000);
		expect(windowed.length).toBeLessThan(40);
	});

	it("gives a tick the same value wherever the window starts", () => {
		// The value is the each-key: if it shifted with the window, scrolling would re-create every node.
		const a = buildMinorTicks(HALF_HOUR, 200, { startSec: 600, endSec: 610 });
		const b = buildMinorTicks(HALF_HOUR, 200, { startSec: 603, endSec: 610 });
		const overlap = a.filter((t) => t >= 603 && t <= 610);
		for (const t of overlap) expect(b).toContain(t);
	});

	it("windowed ticks are a subset of the unwindowed ones", () => {
		const all = new Set(buildMinorTicks(120, 100));
		for (const t of buildMinorTicks(120, 100, { startSec: 30, endSec: 45 })) {
			expect(all.has(t)).toBe(true);
		}
	});

	it("never emits past the duration or before zero", () => {
		const ticks = buildMinorTicks(20, 100, { startSec: -50, endSec: 1000 });
		expect(Math.min(...ticks)).toBe(0);
		expect(Math.max(...ticks)).toBeLessThanOrEqual(21);
	});

	it("windows the labelled markers too, keeping their labels", () => {
		const all = buildTimeMarkers(HALF_HOUR, 200, "smpte", 30);
		const windowed = buildTimeMarkers(HALF_HOUR, 200, "smpte", 30, {
			startSec: 600,
			endSec: 610,
		});

		expect(windowed.length).toBeLessThan(all.length);
		for (const marker of windowed) {
			const match = all.find((m) => m.time === marker.time);
			expect(match?.label).toBe(marker.label);
			expect(match?.emphasis).toBe(marker.emphasis);
		}
	});

	it("returns nothing for a zero-length timeline", () => {
		expect(buildMinorTicks(0, 100)).toEqual([]);
		expect(buildTimeMarkers(0, 100, "smpte", 30)).toEqual([]);
	});
});
