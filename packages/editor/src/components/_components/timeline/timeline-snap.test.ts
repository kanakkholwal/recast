import { describe, expect, it } from "vitest";
import { buildSnapTargets, type SnapTarget, snapTime } from "./timeline-snap";

const fps = 30;

describe("snapTime", () => {
	const targets: SnapTarget[] = [
		{ time: 5, kind: "playhead" },
		{ time: 10, kind: "region-start" },
	];

	it("snaps to a target inside the tolerance", () => {
		const r = snapTime(5.05, targets, 0.1, fps);
		expect(r.time).toBe(5);
		expect(r.target?.kind).toBe("playhead");
	});

	it("picks the nearest target when several are in range", () => {
		const r = snapTime(9.6, targets, 1, fps);
		expect(r.time).toBe(10);
		expect(r.target?.kind).toBe("region-start");
	});

	it("falls through to the frame grid when nothing is in range", () => {
		const r = snapTime(7.013, targets, 0.05, fps);
		expect(r.target).toBeNull();
		// quantised to the nearest 1/30s frame
		expect(r.time).toBeCloseTo(Math.round(7.013 * fps) / fps, 6);
	});

	it("treats a target exactly at the tolerance edge as in range", () => {
		const r = snapTime(5.1, targets, 0.1, fps);
		expect(r.target?.kind).toBe("playhead");
		expect(r.time).toBe(5);
	});

	it("returns the frame-quantised candidate with no targets", () => {
		const r = snapTime(3.337, [], 0.1, fps);
		expect(r.target).toBeNull();
		expect(r.time).toBeCloseTo(Math.round(3.337 * fps) / fps, 6);
	});
});

describe("buildSnapTargets", () => {
	const base = {
		playhead: 4,
		inPoint: 1,
		outPoint: 9,
		duration: 12,
		regions: [{ id: "r1", start: 2, end: 6 }],
		annotations: [{ id: "a1", start: 3, end: 5 }],
	};

	it("always includes playhead, in/out, origin and duration", () => {
		const kinds = buildSnapTargets({ ...base, regions: [], annotations: [] }).map((t) => t.kind);
		expect(kinds).toEqual(["playhead", "in-point", "out-point", "origin", "duration"]);
	});

	it("adds both edges of every region and annotation", () => {
		const targets = buildSnapTargets(base);
		expect(targets).toContainEqual({ time: 2, kind: "region-start" });
		expect(targets).toContainEqual({ time: 6, kind: "region-end" });
		expect(targets).toContainEqual({ time: 3, kind: "annotation-start" });
		expect(targets).toContainEqual({ time: 5, kind: "annotation-end" });
	});

	it("excludes the dragged region's own edges", () => {
		const targets = buildSnapTargets({ ...base, excludeRegionId: "r1" });
		expect(targets.some((t) => t.kind === "region-start")).toBe(false);
		expect(targets.some((t) => t.kind === "region-end")).toBe(false);
		// annotation edges remain
		expect(targets.some((t) => t.kind === "annotation-start")).toBe(true);
	});

	it("excludes the dragged annotation's own edges", () => {
		const targets = buildSnapTargets({ ...base, excludeAnnotationId: "a1" });
		expect(targets.some((t) => t.kind.startsWith("annotation"))).toBe(false);
		expect(targets.some((t) => t.kind === "region-start")).toBe(true);
	});
});
