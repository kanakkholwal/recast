import { describe, expect, it } from "vitest";
import { gainFromPercent, MAX_GAIN, trackGain } from "./audio-gain";

describe("gainFromPercent", () => {
	it("is unity at 100% and silent at 0%", () => {
		expect(gainFromPercent(100)).toBe(1);
		expect(gainFromPercent(0)).toBe(0);
	});

	// The panel's slider runs to 200% and labels it "+6.0 dB" with a Boost badge.
	// Capping the preview at unity made that readout a lie: you could not hear
	// the boost (or the clipping the badge warns about) until after an export.
	it("passes boost above 100% through instead of capping at unity", () => {
		expect(gainFromPercent(200)).toBe(2);
		expect(gainFromPercent(150)).toBeCloseTo(1.5, 6);
	});

	// Mirrors `effective_audio_gain` in commands/editor.rs, which clamps to 0..4.
	it("clamps to the same band the export clamps to", () => {
		expect(gainFromPercent(10_000)).toBe(MAX_GAIN);
		expect(gainFromPercent(-50)).toBe(0);
	});

	it("is silent rather than NaN for a non-finite value", () => {
		expect(gainFromPercent(Number.NaN)).toBe(0);
	});
});

describe("trackGain", () => {
	it("multiplies master by the per-track gain", () => {
		expect(trackGain(50, 50, false, false)).toBeCloseTo(0.25, 6);
	});

	it("compounds boost on both levels", () => {
		expect(trackGain(200, 200, false, false)).toBe(4);
	});

	it("is silent when either level is muted", () => {
		expect(trackGain(100, 100, true, false)).toBe(0);
		expect(trackGain(100, 100, false, true)).toBe(0);
	});
});
