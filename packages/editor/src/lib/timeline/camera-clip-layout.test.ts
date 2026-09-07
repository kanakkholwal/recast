import { describe, expect, it } from "vitest";
import type { CameraLayout } from "../editor/render-state";
import {
	clampSplitFraction,
	DEFAULT_SPLIT_FRACTION,
	layoutAtStart,
	layoutAtTime,
	pruneClipLayouts,
	editAnchor,
	LAYOUT_LABELS,
	layoutLabel,
	segmentStartAt,
	setClipLayout,
	withFraction,
	withSide,
} from "./camera-clip-layout";
import type { Segment } from "./segments";

const segments: Segment[] = [
	{ start: 0, end: 4, index: 0 },
	{ start: 4, end: 9, index: 1 },
	{ start: 9, end: 12, index: 2 },
];

const splitH: CameraLayout = { kind: "splitH", fraction: 0.3, side: "start" };
const splitV: CameraLayout = { kind: "splitV", fraction: 0.4, side: "end" };

describe("layoutAtStart", () => {
	it("is the bubble when the clip has no authored layout", () => {
		expect(layoutAtStart([], 4)).toEqual({ kind: "pip" });
	});

	it("finds the layout anchored at that start", () => {
		expect(layoutAtStart([{ start: 4, layout: splitH }], 4)).toEqual(splitH);
	});

	// Anchors come back through f64 round-trips, so an exact compare would miss.
	it("matches an anchor a float round-trip nudged", () => {
		expect(layoutAtStart([{ start: 4.000_01, layout: splitH }], 4)).toEqual(splitH);
	});
});

describe("layoutAtTime", () => {
	const layouts = [{ start: 4, layout: splitH }];

	it("reads the layout of the clip containing the time", () => {
		expect(layoutAtTime(segments, layouts, 6)).toEqual(splitH);
		expect(layoutAtTime(segments, layouts, 2)).toEqual({ kind: "pip" });
	});

	// Same forward bias as segmentSpeedAtTime, so the panel and the speed control agree on which clip the playhead is in.
	it("resolves a seam onto the following clip", () => {
		expect(layoutAtTime(segments, layouts, 4)).toEqual(splitH);
		expect(layoutAtTime(segments, layouts, 9)).toEqual({ kind: "pip" });
	});

	it("holds the last clip once the time runs past the end", () => {
		expect(layoutAtTime(segments, [{ start: 9, layout: splitV }], 99)).toEqual(splitV);
	});

	it("is the bubble when there are no segments at all", () => {
		expect(layoutAtTime([], layouts, 5)).toEqual({ kind: "pip" });
	});
});

describe("segmentStartAt", () => {
	it("finds the containing clip's anchor", () => {
		expect(segmentStartAt(segments, 6)).toBe(4);
	});

	it("has no anchor to offer without segments", () => {
		expect(segmentStartAt([], 6)).toBeNull();
	});
});

describe("setClipLayout", () => {
	it("anchors the layout at the clip start", () => {
		expect(setClipLayout([], 4, splitH)).toEqual([{ start: 4, layout: splitH }]);
	});

	it("replaces the layout already anchored there", () => {
		const next = setClipLayout([{ start: 4, layout: splitH }], 4, splitV);
		expect(next).toEqual([{ start: 4, layout: splitV }]);
	});

	// An untouched project must serialize to nothing, not gain a clipLayouts array that says "render exactly as before".
	it("removes the entry when a clip goes back to the bubble", () => {
		expect(setClipLayout([{ start: 4, layout: splitH }], 4, { kind: "pip" })).toEqual([]);
	});

	it("keeps the list sorted by anchor", () => {
		let next = setClipLayout([], 9, splitV);
		next = setClipLayout(next, 0, splitH);
		expect(next.map((c) => c.start)).toEqual([0, 9]);
	});

	it("clamps a split that would collapse one half", () => {
		const [entry] = setClipLayout([], 0, { kind: "splitH", fraction: 0.99, side: "start" });
		expect(entry.layout).toMatchObject({ fraction: clampSplitFraction(0.99) });
	});

	it("does not alias the layout it was handed", () => {
		const source: CameraLayout = { kind: "splitH", fraction: 0.3, side: "start" };
		const [entry] = setClipLayout([], 0, source);
		expect(entry.layout).not.toBe(source);
	});
});

describe("pruneClipLayouts", () => {
	// A trim or split that orphans an anchor drops it rather than pinning a layout to a time no clip starts at.
	it("drops an anchor no clip starts at", () => {
		const layouts = [
			{ start: 4, layout: splitH },
			{ start: 7, layout: splitV },
		];
		expect(pruneClipLayouts(layouts, segments).map((c) => c.start)).toEqual([4]);
	});

	it("keeps everything still anchored to a clip", () => {
		const layouts = [{ start: 9, layout: splitV }];
		expect(pruneClipLayouts(layouts, segments)).toHaveLength(1);
	});

	it("has nothing to do for a project with no layouts", () => {
		expect(pruneClipLayouts([], segments)).toEqual([]);
	});
});

describe("clampSplitFraction", () => {
	it("keeps both halves on screen at any input", () => {
		for (const f of [-1, 0, 0.01, 0.99, 1, 5]) {
			const c = clampSplitFraction(f);
			expect(c).toBeGreaterThan(0.1);
			expect(c).toBeLessThan(0.9);
		}
	});

	it("falls back to the default rather than producing NaN", () => {
		expect(clampSplitFraction(Number.NaN)).toBe(DEFAULT_SPLIT_FRACTION);
	});
});

describe("withSide and withFraction", () => {
	it("edit a split without touching its kind", () => {
		expect(withSide(splitH, "end")).toEqual({ ...splitH, side: "end" });
		expect(withFraction(splitV, 0.5)).toEqual({ ...splitV, fraction: 0.5 });
	});

	// The panel keeps these mounted across a layout change, so they must be inert rather than fabricating a split from the bubble.
	it("leave a non-split layout exactly as it was", () => {
		const pip: CameraLayout = { kind: "pip" };
		expect(withSide(pip, "end")).toBe(pip);
		expect(withFraction(pip, 0.5)).toBe(pip);
	});
});

describe("editAnchor", () => {
	// Clicking a camera clip must edit THAT clip, even with the playhead parked elsewhere.
	it("prefers the selected clip over the playhead", () => {
		expect(editAnchor(segments, 9, 1)).toBe(9);
	});

	it("falls back to the playhead when nothing is selected", () => {
		expect(editAnchor(segments, null, 6)).toBe(4);
	});

	// A trim can delete the selected clip; the controls must not keep writing to an anchor no clip starts at.
	it("ignores a selection a trim has since removed", () => {
		expect(editAnchor(segments, 7, 6)).toBe(4);
	});

	it("has no anchor to offer without segments", () => {
		expect(editAnchor([], 4, 6)).toBeNull();
	});
});

describe("layoutLabel", () => {
	// Printed on every camera clip, so a wrong name is the timeline lying about what that clip does.
	it("names each layout distinctly", () => {
		const names = [
			layoutLabel({ kind: "pip" }),
			layoutLabel(splitH),
			layoutLabel(splitV),
			layoutLabel({ kind: "screenOnly" }),
			layoutLabel({ kind: "cameraOnly" }),
		];
		expect(names).toEqual(["Bubble", "Side by side", "Stacked", "Screen only", "Camera only"]);
		expect(new Set(names).size).toBe(names.length);
	});

	it("covers every layout the union allows", () => {
		expect(LAYOUT_LABELS.map((l) => l.kind)).toEqual([
			"pip",
			"splitH",
			"splitV",
			"screenOnly",
			"cameraOnly",
		]);
	});
});
