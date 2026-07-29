import { describe, expect, it } from "vitest";
import {
	drawCaptionLayerExport,
	paintCaptionChunk,
	resolveCaptionView,
	type CaptionView,
} from "./caption-layer-export";
import { buildTimeMap } from "$lib/timeline/time-map";
import {
	DEFAULT_CAPTION_STYLE,
	resolveCaptionAnimation,
	type CaptionStyle,
} from "@recast/captions";

const identity = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 1 }]);

// Structural transcript — resolveCaptionView only reads segments/words.
function transcript() {
	return {
		segments: [
			{
				id: "s1",
				start: 0,
				end: 4,
				text: "hello world",
				words: [
					{ start: 0, end: 1, text: "hello" },
					{ start: 1, end: 2, text: "world" },
				],
			},
		],
	} as never;
}

const style = (over: Partial<CaptionStyle> = {}): CaptionStyle =>
	({ ...DEFAULT_CAPTION_STYLE, enabled: true, animation: undefined, ...over }) as CaptionStyle;

describe("resolveCaptionView", () => {
	it("returns the active line inside a segment (static → all spoken)", () => {
		const v = resolveCaptionView(transcript(), style(), identity, 1);
		expect(v).not.toBeNull();
		expect(v?.words.map((w) => w.text)).toEqual(["hello", "world"]);
		expect(v?.spoken).toBe(2);
		expect(v?.activeIndex).toBe(-1);
	});

	it("returns null past the segment end (nothing on screen)", () => {
		expect(resolveCaptionView(transcript(), style(), identity, 5)).toBeNull();
	});

	it("returns null when captions are disabled or there's no transcript", () => {
		expect(resolveCaptionView(transcript(), style({ enabled: false }), identity, 1)).toBeNull();
		expect(resolveCaptionView(null, style(), identity, 1)).toBeNull();
	});

	it("returns null inside a cut (playhead in the gap between kept spans)", () => {
		const cut = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 5, origEnd: 10, speed: 1 },
		]);
		expect(resolveCaptionView(transcript(), style(), cut, 3)).toBeNull();
	});
});

// Recording mock of the 2D surface paintCaptionChunk touches.
function mockCtx(measure = (s: string) => ({ width: s.length * 10 })) {
	const calls: string[] = [];
	const rec =
		(name: string) =>
		(...args: unknown[]) =>
			calls.push(args.length ? `${name}(${args.length})` : name);
	return {
		calls,
		canvas: { width: 1920, height: 1080 },
		font: "",
		letterSpacing: "",
		textBaseline: "alphabetic",
		globalAlpha: 1,
		fillStyle: "",
		strokeStyle: "",
		lineWidth: 0,
		lineJoin: "miter",
		shadowColor: "",
		shadowBlur: 0,
		shadowOffsetY: 0,
		measureText: (s: string) => measure(s),
		save: rec("save"),
		restore: rec("restore"),
		translate: rec("translate"),
		scale: rec("scale"),
		beginPath: rec("beginPath"),
		closePath: rec("closePath"),
		moveTo: rec("moveTo"),
		lineTo: rec("lineTo"),
		quadraticCurveTo: rec("quadraticCurveTo"),
		fill: rec("fill"),
		fillText: rec("fillText"),
		strokeText: rec("strokeText"),
	};
}

const view = (over: Partial<CaptionView> = {}): CaptionView => ({
	words: [
		{ start: 0, end: 1, text: "hello" },
		{ start: 1, end: 2, text: "world" },
	],
	spoken: 2,
	activeIndex: -1,
	chunkStart: 0,
	anim: resolveCaptionAnimation(undefined),
	...over,
});

const fullFrame = {
	videoLeftFrac: 0,
	videoRightFrac: 1,
	videoTopFrac: 0,
	videoBottomFrac: 1,
	canvasPxW: 1920,
	canvasPxH: 1080,
};

describe("paintCaptionChunk", () => {
	it("fills the pill and draws each word", () => {
		const ctx = mockCtx();
		paintCaptionChunk(ctx as never, view(), style({ background: "box" }), 5, fullFrame);
		expect(ctx.calls).toContain("fill"); // pill background
		expect(ctx.calls.filter((c) => c === "fillText(3)")).toHaveLength(2); // two words
	});

	it("strokes an outline under the fill when outlineWidth > 0", () => {
		const ctx = mockCtx();
		paintCaptionChunk(ctx as never, view(), style({ outlineWidth: 8 }), 5, fullFrame);
		expect(ctx.calls).toContain("strokeText(3)");
	});

	it("does not fill a pill for a background-less caption", () => {
		const ctx = mockCtx();
		paintCaptionChunk(ctx as never, view(), style({ background: "none" }), 5, fullFrame);
		expect(ctx.calls).not.toContain("fill");
		expect(ctx.calls.filter((c) => c === "fillText(3)")).toHaveLength(2);
	});
});

describe("drawCaptionLayerExport", () => {
	it("draws nothing when no caption is active", () => {
		const ctx = mockCtx();
		drawCaptionLayerExport(ctx as never, 5, {
			transcript: transcript(),
			style: style(),
			timeMap: identity,
			video: { leftFrac: 0, rightFrac: 1, topFrac: 0, bottomFrac: 1 },
			canvasPxW: 1920,
			canvasPxH: 1080,
		});
		expect(ctx.calls).toEqual([]);
	});
});
