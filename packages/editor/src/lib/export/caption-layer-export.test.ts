import { describe, expect, it } from "vitest";
import { drawCaptionLayerExport } from "./caption-layer-export";
import {
	captionClocks,
	paintCaptionChunk,
	resolveCaptionView,
	type CaptionView,
} from "../captions/caption-render";
import { buildTimeMap } from "../timeline/time-map";
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

// `store.currentTime` is ORIGINAL (source) time. The preview overlay must resolve
// the chunk at that source time directly and clock the entrance on OUTPUT time —
// NOT double-convert through outputToOriginal (the inverted-axis bug that made
// highlight/emphasis/entrance never reflect on a trimmed/sped timeline).
describe("captionClocks", () => {
	it("passes source time through and derives output time (identity map)", () => {
		const c = captionClocks(identity, 3);
		expect(c.sourceSec).toBe(3);
		expect(c.outputSec).toBeCloseTo(3, 6);
	});

	it("keeps source time but shifts the entrance clock under a trim (non-identity map)", () => {
		// One kept span [3,13] → output [0,10]: playhead at source 5 is output 2.
		const trimmed = buildTimeMap([{ origStart: 3, origEnd: 13, speed: 1 }]);
		const c = captionClocks(trimmed, 5);
		expect(c.sourceSec).toBe(5); // resolve words at the true source time
		expect(c.outputSec).toBeCloseTo(2, 6); // entrance runs on viewer/output time
		// The old inverted formula (outputToOriginal for the source) would have
		// resolved at 8 — 3s ahead — killing per-word highlight and entrance.
	});

	it("compresses the entrance clock on a sped-up span (viewer-rate)", () => {
		const sped = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 2 }]);
		const c = captionClocks(sped, 6); // 6s of source at 2× = 3s of output
		expect(c.sourceSec).toBe(6);
		expect(c.outputSec).toBeCloseTo(3, 6);
	});
});

// The "size" active-word emphasis must EASE in/out over time (a per-word bump),
// not snap binary 1→1.14→1 per word — the hard pop reads as jitter/overlap.
describe("emphasis scale (smooth pop)", () => {
	const scaleStyle = () =>
		style({
			animation: {
				chunk: "line",
				chunkSize: 3,
				emphasis: "scale",
				emphasisColor: "#ffffff",
				highlight: "none",
				entrance: "none",
				entranceMs: 200,
				holdGaps: true,
			},
		});

	it("pops the active word and leaves the not-yet-spoken word unscaled", () => {
		const v = resolveCaptionView(transcript(), scaleStyle(), identity, 0.5); // mid "hello" [0,1]
		expect(v?.wordScales?.[0]).toBeGreaterThan(1.05); // active → popped up
		expect(v?.wordScales?.[1]).toBeCloseTo(1, 2); // "world" not started → unscaled
	});

	it("ramps the scale (not a binary jump) just after the word starts", () => {
		const full =
			resolveCaptionView(transcript(), scaleStyle(), identity, 0.5)?.wordScales?.[0] ?? 0;
		const rising =
			resolveCaptionView(transcript(), scaleStyle(), identity, 0.02)?.wordScales?.[0] ?? 0;
		expect(rising).toBeGreaterThan(1); // already lifting off
		expect(rising).toBeLessThan(full); // but below full — proves the ease-in, not a snap
	});

	it("carries no scales when emphasis isn't 'scale'", () => {
		expect(resolveCaptionView(transcript(), style(), identity, 0.5)?.wordScales).toBeUndefined();
	});
});

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

	it("ignores the preview `enabled` toggle (the caller gates); null only without a transcript", () => {
		// Export burn keys on burnIn, not the preview-visibility toggle — the pure
		// resolver must not swallow the caption when `enabled` is false.
		expect(resolveCaptionView(transcript(), style({ enabled: false }), identity, 1)).not.toBeNull();
		expect(resolveCaptionView(null, style(), identity, 1)).toBeNull();
	});

	it("resolves an animated (chunked / progressive) caption to a valid chunk", () => {
		const s = style({
			animation: {
				chunk: "word",
				chunkSize: 1,
				emphasis: "none",
				emphasisColor: "#ffffff",
				highlight: "progressive",
				entrance: "pop",
				entranceMs: 300,
				holdGaps: true,
			},
		});
		const v = resolveCaptionView(transcript(), s, identity, 1.5); // within "world" [1,2]
		expect(v).not.toBeNull();
		expect(v?.words.map((w) => w.text)).toEqual(["world"]);
	});

	it("returns null inside a cut (playhead in the gap between kept spans)", () => {
		const cut = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 5, origEnd: 10, speed: 1 },
		]);
		expect(resolveCaptionView(transcript(), style(), cut, 3)).toBeNull();
	});

	// A split (or speed change) is a span boundary with NO removed content, so a
	// caption spanning it must keep ALL its words — only real cuts clip captions.
	function spanningTranscript() {
		return {
			segments: [
				{
					id: "s1",
					start: 4,
					end: 7,
					text: "a b c",
					words: [
						{ start: 4, end: 5, text: "a" },
						{ start: 5, end: 6, text: "b" },
						{ start: 6, end: 7, text: "c" },
					],
				},
			],
		} as never;
	}

	it("keeps words across a split boundary (contiguous spans, no cut)", () => {
		const split = buildTimeMap([
			{ origStart: 0, origEnd: 5, speed: 1 },
			{ origStart: 5, origEnd: 10, speed: 1 },
		]);
		const v = resolveCaptionView(spanningTranscript(), style(), split, 4.5);
		expect(v?.words.map((w) => w.text)).toEqual(["a", "b", "c"]);
	});

	it("keeps words across a speed-change boundary (still no cut)", () => {
		const speed = buildTimeMap([
			{ origStart: 0, origEnd: 5, speed: 1 },
			{ origStart: 5, origEnd: 10, speed: 2 },
		]);
		const v = resolveCaptionView(spanningTranscript(), style(), speed, 5.5);
		expect(v?.words.map((w) => w.text)).toEqual(["a", "b", "c"]);
	});

	it("shows the full caption on each side of a cut, but nothing inside the gap", () => {
		const cut = buildTimeMap([
			{ origStart: 0, origEnd: 5, speed: 1 },
			{ origStart: 6, origEnd: 10, speed: 1 },
		]);
		// A caption straddling the cut keeps ALL its words on each side (reliable),
		// rather than dropping the far-side ones and reading as gone.
		expect(
			resolveCaptionView(spanningTranscript(), style(), cut, 4.5)?.words.map((w) => w.text),
		).toEqual(["a", "b", "c"]);
		// ...and it correctly disappears while the playhead is inside the cut gap.
		expect(resolveCaptionView(spanningTranscript(), style(), cut, 5.5)).toBeNull();
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
	chunkStartOutput: 0,
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
		drawCaptionLayerExport(ctx as never, 5, 5, {
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
