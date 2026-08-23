import { describe, expect, it } from "vitest";
import { ditherStrengthFor, orderedDither } from "./gif-dither";

function solid(width: number, height: number, v: number): Uint8ClampedArray {
	const out = new Uint8ClampedArray(width * height * 4);
	for (let i = 0; i < out.length; i += 4) {
		out[i] = v;
		out[i + 1] = v;
		out[i + 2] = v;
		out[i + 3] = 255;
	}
	return out;
}

describe("orderedDither", () => {
	it("does not mutate the input, so the palette can come from clean pixels", () => {
		const src = solid(8, 8, 128);
		const copy = Uint8ClampedArray.from(src);
		orderedDither(src, 8, 8, 16);
		expect(src).toEqual(copy);
	});

	it("leaves alpha untouched", () => {
		const out = orderedDither(solid(8, 8, 128), 8, 8, 32);
		for (let i = 3; i < out.length; i += 4) expect(out[i]).toBe(255);
	});

	it("spreads a flat field across several values, which is what kills banding", () => {
		const out = orderedDither(solid(8, 8, 128), 8, 8, 16);
		const seen = new Set<number>();
		for (let i = 0; i < out.length; i += 4) seen.add(out[i]!);
		expect(seen.size).toBeGreaterThan(4);
	});

	it("keeps the nudge inside the requested strength", () => {
		const strength = 16;
		const out = orderedDither(solid(8, 8, 128), 8, 8, strength);
		for (let i = 0; i < out.length; i += 4) {
			expect(Math.abs(out[i]! - 128)).toBeLessThanOrEqual(strength / 2 + 1);
		}
	});

	it("is a pass-through at zero strength", () => {
		const src = solid(4, 4, 200);
		expect(orderedDither(src, 4, 4, 0)).toEqual(src);
	});

	it("is position-based, so the same pixel dithers the same way every frame", () => {
		const a = orderedDither(solid(8, 8, 128), 8, 8, 16);
		const b = orderedDither(solid(8, 8, 128), 8, 8, 16);
		expect(a).toEqual(b);
	});

	it("clamps rather than wrapping at the ends of the range", () => {
		const dark = orderedDither(solid(8, 8, 2), 8, 8, 32);
		const light = orderedDither(solid(8, 8, 253), 8, 8, 32);
		for (let i = 0; i < dark.length; i += 4) {
			expect(dark[i]).toBeGreaterThanOrEqual(0);
			expect(light[i]).toBeLessThanOrEqual(255);
		}
	});
});

describe("ditherStrengthFor", () => {
	it("nudges harder as the palette gets coarser", () => {
		expect(ditherStrengthFor(256)).toBeLessThan(ditherStrengthFor(128));
		expect(ditherStrengthFor(128)).toBeLessThan(ditherStrengthFor(64));
		expect(ditherStrengthFor(64)).toBeLessThan(ditherStrengthFor(16));
	});
});
