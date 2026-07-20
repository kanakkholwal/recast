import { describe, expect, it } from "vitest";
import { frameBudget, textureRingFrames } from "../src/cache/frame-budget";

describe("textureRingFrames", () => {
	it("buffers more than the decoder-surface budget allows at 4K", () => {
		// The point of owning textures: decoder surfaces cap at 4 frames at 4K,
		// which is ~66ms of buffer and stalls on any hiccup.
		expect(textureRingFrames(3840, 2160)).toBeGreaterThan(frameBudget(3840, 2160).cacheMax);
	});

	it("scales down with pixel count", () => {
		expect(textureRingFrames(1920, 1080)).toBeGreaterThan(textureRingFrames(3840, 2160));
	});

	it("stays bounded at both extremes", () => {
		expect(textureRingFrames(320, 240)).toBeLessThanOrEqual(16);
		expect(textureRingFrames(7680, 4320)).toBeGreaterThanOrEqual(4);
	});

	it("falls back to the max when dimensions are unknown", () => {
		expect(textureRingFrames(0, 0)).toBe(16);
	});
});

describe("frameBudget", () => {
	it("keeps the historical 7/4/6 budget at common resolutions (≤1440p)", () => {
		expect(frameBudget(1280, 720)).toEqual({ cacheMax: 7, holdoutMax: 4, decodeAhead: 6 });
		expect(frameBudget(1920, 1080)).toEqual({ cacheMax: 7, holdoutMax: 4, decodeAhead: 6 });
		expect(frameBudget(2560, 1440)).toEqual({ cacheMax: 7, holdoutMax: 4, decodeAhead: 6 });
	});

	it("tightens the budget at 4K and 5K to avoid surface-pool starvation", () => {
		const k4 = frameBudget(3840, 2160);
		expect(k4).toEqual({ cacheMax: 4, holdoutMax: 2, decodeAhead: 3 });
		const k5 = frameBudget(5120, 2880);
		expect(k5).toEqual({ cacheMax: 4, holdoutMax: 2, decodeAhead: 3 });
	});

	it("is monotonic: more pixels never increases any cap", () => {
		const sizes: Array<[number, number]> = [
			[1280, 720],
			[1920, 1080],
			[2560, 1440],
			[3840, 2160],
			[5120, 2880],
			[7680, 4320],
		];
		const budgets = sizes.map(([w, h]) => frameBudget(w, h));
		for (let i = 1; i < budgets.length; i++) {
			expect(budgets[i].cacheMax).toBeLessThanOrEqual(budgets[i - 1].cacheMax);
			expect(budgets[i].holdoutMax).toBeLessThanOrEqual(budgets[i - 1].holdoutMax);
			expect(budgets[i].decodeAhead).toBeLessThanOrEqual(budgets[i - 1].decodeAhead);
		}
	});

	it("never drops below safe floors", () => {
		const huge = frameBudget(7680, 4320); // 8K
		expect(huge.cacheMax).toBeGreaterThanOrEqual(4);
		expect(huge.holdoutMax).toBeGreaterThanOrEqual(2);
		expect(huge.decodeAhead).toBeGreaterThanOrEqual(3);
	});

	it("falls back to the generous budget when dimensions are unknown", () => {
		expect(frameBudget(0, 0)).toEqual({ cacheMax: 7, holdoutMax: 4, decodeAhead: 6 });
	});
});
