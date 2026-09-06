import { describe, expect, it } from "vitest";
import {
	buildPressEvents,
	clickAnchorAt,
	clickHighlightAt,
	pressStateAt,
} from "../../components/cursor-animation.logic";
import { idleAlphaAt, interpolateCursor } from "../../components/video-preview.logic";
import { type CursorSampleLike, smoothCursorPath } from "../cursor/smoothing";
import fixture from "./__fixtures__/cursor-parity.json";

/** The same fixture is asserted by `crates/recast-cursor/tests/parity.rs`. If the
 *  two files disagree, the preview and the export put the cursor in different
 *  places, which is the failure this pair exists to make impossible. */
const samples = fixture.samples as CursorSampleLike[];
const smoothed = smoothCursorPath(samples, fixture.smoothing).samples;
const pressEvents = buildPressEvents(samples);

function at(us: number) {
	const sample = smoothed.find((s) => s.timestampUs === us);
	if (!sample) throw new Error(`no smoothed sample at ${us}`);
	return sample;
}

function resolve(tsUs: number) {
	const idle = idleAlphaAt(fixture.idlePeriods, tsUs, fixture.settings.idleTimeout);
	const press = pressStateAt(pressEvents, tsUs);
	return {
		alpha: Math.max(idle, press.visibleAlpha),
		press,
		pos: interpolateCursor(smoothed, null, tsUs),
		anchor: clickAnchorAt(pressEvents, tsUs),
		highlight: clickHighlightAt(pressEvents, tsUs),
	};
}

describe("cursor smoothing", () => {
	it("pulls each sample toward its Gaussian neighbourhood", () => {
		expect(at(0).x).toBeCloseTo(158.197716913, 6);
		expect(at(0).y).toBeCloseTo(116.231687949, 6);
		expect(at(208000).x).toBeCloseTo(455.89255421, 6);
		expect(at(208000).y).toBeCloseTo(268.880114936, 6);
	});

	/** Without the snap, smoothing rounds the corner through a click and the
	 *  pointer lands somewhere the user never clicked. */
	it("anchors the path exactly onto every click position", () => {
		expect([at(80000).x, at(80000).y]).toEqual([300, 160]);
		expect([at(176000).x, at(176000).y]).toEqual([455, 268]);
	});

	it("leaves a run of identical samples untouched", () => {
		expect([at(400000).x, at(400000).y]).toEqual([480, 288]);
	});
});

describe("press events", () => {
	it("pairs each rising edge with its release and classifies the button", () => {
		expect(pressEvents).toEqual([
			{ downUs: 80000, upUs: 112000, downX: 300, downY: 160, right: false, dragged: false },
			{ downUs: 176000, upUs: 208000, downX: 455, downY: 268, right: true, dragged: true },
		]);
	});
});

describe("per-frame resolve", () => {
	it("telegraphs the press before the click lands", () => {
		const before = resolve(0);
		expect(before.press.pressedSprite).toBe(true);
		expect(before.press.scale).toBeCloseTo(1.01574344, 6);
		expect(before.anchor?.weight).toBeCloseTo(0.654508497, 6);
	});

	it("snaps to the punch scale on the click frame", () => {
		expect(resolve(80000).press.scale).toBeCloseTo(0.84, 6);
		expect(resolve(200000).press.scale).toBeCloseTo(0.845872576, 6);
	});

	/** The highlight starts at zero on the impact frame and ramps in over 40 ms,
	 *  so asserting "visible at the click" would pass on a broken envelope. */
	it("fades the highlight in from nothing at the click", () => {
		expect(resolve(80000).highlight?.alpha).toBe(0);
		expect(resolve(88000).highlight?.alpha).toBeCloseTo(0.104, 9);
		expect(resolve(130000).highlight?.alpha).toBe(1);
		expect(resolve(900000).highlight).toBeNull();
	});

	it("keeps a press visible even while idle-hide has faded to zero", () => {
		expect(idleAlphaAt(fixture.idlePeriods, 560000, fixture.settings.idleTimeout)).toBeCloseTo(
			0.82,
			9,
		);
		expect(resolve(560000).alpha).toBe(1);
		expect(resolve(900000).alpha).toBeCloseTo(0.797849108, 6);
	});

	/** Booleans flip at the midpoint of the LINEAR parameter, so an invisible
	 *  sample keeps the cursor hidden for the first half of its span. */
	it("takes visibility from the nearer sample rather than interpolating it", () => {
		expect(resolve(165000).pos?.visible).toBe(false);
		expect(resolve(200000).pos?.visible).toBe(true);
	});

	it("interpolates position between captured samples", () => {
		expect(resolve(24000).pos?.x).toBeCloseTo(208.926806144, 6);
		expect(resolve(24000).pos?.y).toBeCloseTo(132.126505975, 6);
	});

	it("drops the anchor once the snap window has passed", () => {
		expect(resolve(560000).anchor).toBeNull();
	});
});
