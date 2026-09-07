import { describe, expect, it } from "vitest";
import { heardOutputSec } from "./audio-engine";

describe("heardOutputSec", () => {
	it("reports the position the listener is hearing, not the one submitted", () => {
		// 2s of context time elapsed, but 200ms sits in the hardware buffer, so only 1.8s has actually been heard.
		expect(heardOutputSec(0, 10, 12, 0.2)).toBeCloseTo(1.8, 6);
	});

	it("matches the raw clock when the device reports no latency", () => {
		expect(heardOutputSec(0, 10, 12, 0)).toBeCloseTo(2, 6);
	});

	it("carries the output anchor through", () => {
		expect(heardOutputSec(30, 10, 12, 0.2)).toBeCloseTo(31.8, 6);
	});

	it("never reports a position before the anchor", () => {
		// Right after scheduling nothing has been heard yet, and a negative position would drag the picture backwards.
		expect(heardOutputSec(5, 10, 10.05, 0.2)).toBe(5);
	});

	it("ignores a non-finite or negative latency", () => {
		expect(heardOutputSec(0, 10, 12, Number.NaN)).toBeCloseTo(2, 6);
		expect(heardOutputSec(0, 10, 12, -1)).toBeCloseTo(2, 6);
	});

	it("exceeds the resync threshold on a Bluetooth-class device", () => {
		// 150ms of uncompensated latency is far past the 60ms threshold, so the picture would sit permanently ahead.
		const uncompensated = heardOutputSec(0, 10, 12, 0);
		const compensated = heardOutputSec(0, 10, 12, 0.15);
		expect(uncompensated - compensated).toBeGreaterThan(0.06);
	});
});
