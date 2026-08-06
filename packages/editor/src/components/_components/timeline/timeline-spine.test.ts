import { describe, expect, it } from "vitest";
import {
	applySpineHandle,
	buildSpineHandles,
	canSlip,
	planSlip,
	type SpineShape,
} from "./timeline-spine.logic";

const FPS = 60;
// 2 frames / 100ms, the values the clip bar and cut lane use.
const MIN_CLIP = 2 / FPS;
const MIN_CUT = 0.1;

function shape(
	segments: Array<[number, number]>,
	cuts: Array<[string, number, number]> = [],
): SpineShape {
	return {
		segments: segments.map(([start, end]) => ({ start, end })),
		cuts: cuts.map(([id, start, end]) => ({ id, start, end })),
		minClip: MIN_CLIP,
		minCut: MIN_CUT,
	};
}

describe("buildSpineHandles", () => {
	it("has nothing to offer on an unsplit, uncut clip", () => {
		expect(buildSpineHandles(shape([[0, 10]]))).toEqual([]);
	});

	it("puts a roll handle on a split boundary", () => {
		const handles = buildSpineHandles(
			shape([
				[0, 4],
				[4, 10],
			]),
		);
		expect(handles).toHaveLength(1);
		expect(handles[0]).toMatchObject({ kind: "roll", at: 4, leftIndex: 0, rightIndex: 1 });
	});

	it("bounds a roll by both neighbours' minimum length", () => {
		const [roll] = buildSpineHandles(
			shape([
				[0, 4],
				[4, 10],
			]),
		);
		expect(roll.min).toBeCloseTo(MIN_CLIP);
		expect(roll.max).toBeCloseTo(10 - MIN_CLIP);
	});

	it("puts a slide handle on a seam and reserves the removed length", () => {
		const handles = buildSpineHandles(
			shape(
				[
					[0, 4],
					[6, 10],
				],
				[["c1", 4, 6]],
			),
		);
		expect(handles).toHaveLength(1);
		expect(handles[0]).toMatchObject({ kind: "slide", at: 4, cutId: "c1", cutLength: 2 });
		// The right block must keep minClip AFTER the 2s window lands past it.
		expect(handles[0].max).toBeCloseTo(10 - MIN_CLIP - 2);
	});

	it("declines a boundary with no room to move", () => {
		// Both blocks are already at the minimum length.
		const tiny = MIN_CLIP;
		expect(
			buildSpineHandles(
				shape([
					[0, tiny],
					[tiny, tiny * 2],
				]),
			),
		).toEqual([]);
	});

	// Two un-merged cuts can share a gap between drags; moving "the" cut would
	// silently pick one and leave the other behind.
	it("declines a seam filled by more than one cut", () => {
		const handles = buildSpineHandles(
			shape(
				[
					[0, 4],
					[8, 12],
				],
				[
					["c1", 4, 6],
					["c2", 6, 8],
				],
			),
		);
		expect(handles).toEqual([]);
	});

	it("emits one handle per interior boundary, in order", () => {
		const handles = buildSpineHandles(
			shape(
				[
					[0, 4],
					[4, 6],
					[8, 12],
				],
				[["c1", 6, 8]],
			),
		);
		expect(handles.map((h) => h.kind)).toEqual(["roll", "slide"]);
		expect(handles.map((h) => h.at)).toEqual([4, 6]);
	});
});

describe("applySpineHandle", () => {
	it("rolls a split to the dragged time", () => {
		const [roll] = buildSpineHandles(
			shape([
				[0, 4],
				[4, 10],
			]),
		);
		expect(applySpineHandle(roll, 5.5, FPS)).toEqual({ kind: "roll", from: 4, to: 5.5 });
	});

	it("clamps a roll inside both neighbours", () => {
		const [roll] = buildSpineHandles(
			shape([
				[0, 4],
				[4, 10],
			]),
		);
		expect((applySpineHandle(roll, -3, FPS) as { to: number }).to).toBeCloseTo(MIN_CLIP);
		expect((applySpineHandle(roll, 99, FPS) as { to: number }).to).toBeCloseTo(10 - MIN_CLIP);
	});

	it("moves a slide's whole window, keeping the removed length", () => {
		const [slide] = buildSpineHandles(
			shape(
				[
					[0, 4],
					[6, 10],
				],
				[["c1", 4, 6]],
			),
		);
		expect(applySpineHandle(slide, 5, FPS)).toEqual({
			kind: "slide",
			cutId: "c1",
			start: 5,
			end: 7,
		});
	});

	it("lands on the frame grid", () => {
		const [roll] = buildSpineHandles(
			shape([
				[0, 4],
				[4, 10],
			]),
		);
		const { to } = applySpineHandle(roll, 5.008333, FPS) as { to: number };
		expect(to * FPS).toBeCloseTo(Math.round(to * FPS));
	});
});

describe("planSlip", () => {
	// A block with a removed range either side: the only shape that has slack.
	const slippable = shape(
		[
			[0, 4],
			[6, 10],
			[12, 16],
		],
		[
			["a", 4, 6],
			["b", 10, 12],
		],
	);

	it("shifts the block and hands the slack to both cuts", () => {
		const plan = planSlip(slippable, 1, 0.5, FPS);
		expect(plan).not.toBeNull();
		expect(plan?.delta).toBeCloseTo(0.5);
		expect(plan?.before).toMatchObject({ id: "a", start: 4, end: 6.5 });
		expect(plan?.after).toMatchObject({ id: "b", start: 10.5, end: 12 });
	});

	it("clamps so neither removed range drops below the minimum", () => {
		const plan = planSlip(slippable, 1, 99, FPS);
		// Forward slack ends where cut "b" is down to minCut.
		expect(plan?.delta).toBeCloseTo(12 - MIN_CUT - 10);
		const backwards = planSlip(slippable, 1, -99, FPS);
		expect(backwards?.delta).toBeCloseTo(4 + MIN_CUT - 6);
	});

	it("refuses a block bounded by a split", () => {
		const withSplit = shape(
			[
				[0, 4],
				[4, 8],
				[10, 14],
			],
			[["b", 8, 10]],
		);
		expect(planSlip(withSplit, 1, 0.5, FPS)).toBeNull();
	});

	it("refuses the first and last block", () => {
		expect(planSlip(slippable, 0, 0.5, FPS)).toBeNull();
		expect(planSlip(slippable, 2, 0.5, FPS)).toBeNull();
	});

	it("refuses a shift too small to change a frame", () => {
		expect(planSlip(slippable, 1, 0, FPS)).toBeNull();
	});

	it("agrees with canSlip", () => {
		expect(canSlip(slippable, 1)).toBe(true);
		expect(canSlip(slippable, 0)).toBe(false);
		expect(canSlip(shape([[0, 10]]), 0)).toBe(false);
	});
});
