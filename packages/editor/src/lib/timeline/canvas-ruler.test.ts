import { describe, expect, it } from "vitest";
import type { TimelineView } from "./canvas-view";
import {
	chooseTickInterval,
	formatTickLabel,
	niceIntervals,
	TARGET_MAJOR_TICK_DISTANCE,
	visibleTicks,
} from "./canvas-ruler";

const view = (scrollFrames: number, resolution: number): TimelineView => ({
	scrollFrames,
	resolution,
});

describe("canvas-ruler", () => {
	it("builds a sorted, deduped ladder that includes one second", () => {
		const ladder = niceIntervals(30);
		expect(ladder).toEqual([...ladder].sort((a, b) => a - b));
		expect(new Set(ladder).size).toBe(ladder.length);
		expect(ladder).toContain(30); // one second at 30 fps
	});

	it("picks a major interval near the target spacing, never below it", () => {
		const fps = 60;
		for (const resolution of [0.05, 0.5, 2, 10, 60]) {
			const { major } = chooseTickInterval(view(0, resolution), fps);
			expect(major * resolution).toBeGreaterThanOrEqual(TARGET_MAJOR_TICK_DISTANCE - 1e-6);
		}
	});

	it("subdivides into readable minor ticks that divide the major", () => {
		const { major, minor } = chooseTickInterval(view(0, 2), 60);
		expect(major % minor).toBe(0);
		expect(minor * 2).toBeLessThanOrEqual(major); // at least a 2x subdivision when room allows
	});

	it("emits ticks only within the content and viewport, flagging majors", () => {
		const v = view(0, 2);
		const { ticks, interval } = visibleTicks(v, 800, 60, 6000);
		expect(ticks.length).toBeGreaterThan(0);
		for (const t of ticks) {
			expect(t.frame).toBeGreaterThanOrEqual(0);
			expect(t.frame).toBeLessThanOrEqual(6000);
			expect(t.x).toBeGreaterThanOrEqual(-interval.minor * v.resolution - 1e-6);
			expect(t.major).toBe(t.frame % interval.major === 0);
		}
	});

	it("does not run past a tiny content length", () => {
		const { ticks } = visibleTicks(view(0, 2), 800, 60, 10);
		expect(ticks.every((t) => t.frame <= 10)).toBe(true);
	});

	it("labels second-scale majors as M:SS and sub-second as seconds", () => {
		expect(formatTickLabel(90, 30, 30)).toBe("0:03");
		expect(formatTickLabel(1830, 30, 300)).toBe("1:01");
		expect(formatTickLabel(15, 30, 5)).toBe("0.50s");
	});
});
