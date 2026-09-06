import { describe, expect, it } from "vitest";
import { frameDelta, isWithin } from "../test/golden/delta";
import { GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN } from "../test/golden/tolerance";

/** A frame of `count` pixels, every channel `value`. */
function flat(count: number, value: number): Uint8Array {
	return new Uint8Array(count * 4).fill(value);
}

describe("frameDelta", () => {
	it("reports nothing for identical frames", () => {
		const frame = flat(16, 7);
		expect(frameDelta(frame, frame)).toEqual({
			maxChannel: 0,
			meanChannel: 0,
			differingPixels: 0,
			totalPixels: 16,
		});
	});

	it("refuses frames it cannot compare rather than returning a passing zero", () => {
		expect(() => frameDelta(flat(4, 0), flat(5, 0))).toThrow();
		expect(() => frameDelta(new Uint8Array(0), new Uint8Array(0))).toThrow();
		expect(() => frameDelta(new Uint8Array(6), new Uint8Array(6))).toThrow();
	});

	it("counts a pixel once however many of its channels moved", () => {
		const a = flat(4, 0);
		const b = flat(4, 0);
		b[0] = 10;
		b[1] = 10;
		const delta = frameDelta(a, b);
		expect(delta.differingPixels).toBe(1);
		expect(delta.maxChannel).toBe(10);
		expect(delta.meanChannel).toBe(20 / 16);
	});

	/** Alpha-only drift is the case a naive RGB comparison misses entirely. */
	it("sees a difference that is only in alpha", () => {
		const a = flat(4, 255);
		const b = flat(4, 255);
		b[3] = 0;
		expect(frameDelta(a, b).differingPixels).toBe(1);
		expect(frameDelta(a, b).maxChannel).toBe(255);
	});

	/**
	 * The same claim the native arm makes about its own tolerance: shift one row
	 * by a single pixel and the gate must reject it. A tolerance that survives
	 * this turns the whole golden set into decoration.
	 */
	it("the golden tolerance is tight enough to catch a one-pixel row shift", () => {
		const width = 64;
		const height = 64;
		const frame = new Uint8Array(width * height * 4);
		for (let i = 0; i < frame.length; i += 4) {
			// A vertical stripe pattern, so a horizontal shift actually moves edges.
			const x = (i / 4) % width;
			const on = Math.floor(x / 4) % 2 === 0;
			frame.set(on ? [220, 40, 40, 255] : [40, 200, 90, 255], i);
		}
		const nudged = new Uint8Array(frame);
		const row = Math.floor(height / 2) * width * 4;
		nudged.copyWithin(row + 4, row, row + width * 4 - 4);

		const delta = frameDelta(frame, nudged);
		expect(delta.differingPixels).toBeGreaterThan(0);
		expect(
			isWithin(delta, GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN),
			`a one-pixel shift measured max ${delta.maxChannel} mean ${delta.meanChannel}, which the tolerance lets through`,
		).toBe(false);
	});

	/**
	 * The shift above is rejected on the MEAN alone, so on its own it says nothing
	 * about the channel bound: widening that to 255 still passed it. Each number
	 * needs a case where it is the one doing the work.
	 */
	it("the channel bound rejects a few badly wrong pixels", () => {
		const a = flat(64 * 64, 0);
		const b = flat(64 * 64, 0);
		b[0] = 200;
		const delta = frameDelta(a, b);
		expect(delta.meanChannel, "too many pixels for the mean to be the binding check").toBeLessThan(
			GOLDEN_MAX_MEAN,
		);
		expect(isWithin(delta, GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN)).toBe(false);
	});

	it("the mean bound rejects a wash that no single pixel breaks", () => {
		const a = flat(64 * 64, 10);
		const b = flat(64 * 64, 10 + GOLDEN_MAX_CHANNEL);
		const delta = frameDelta(a, b);
		expect(
			delta.maxChannel,
			"too large for the channel bound to be the binding check",
		).toBeLessThanOrEqual(GOLDEN_MAX_CHANNEL);
		expect(isWithin(delta, GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN)).toBe(false);
	});
});
