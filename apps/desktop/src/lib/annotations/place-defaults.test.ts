import { describe, expect, it } from "vitest";
import { clickPlacedArrow, clickPlacedBox } from "./place-defaults";

const WIDE = { w: 1920, h: 1080 };
const TALL = { w: 1080, h: 1920 };

describe("clickPlacedBox", () => {
	it("centres on the click", () => {
		const b = clickPlacedBox(0.5, 0.5, WIDE.w, WIDE.h);
		expect(b.x + b.w / 2).toBeCloseTo(0.5, 6);
		expect(b.y + b.h / 2).toBeCloseTo(0.5, 6);
	});

	it("stays inside the frame at every corner", () => {
		for (const [ux, uy] of [
			[0, 0],
			[1, 1],
			[0.01, 0.99],
		]) {
			for (const f of [WIDE, TALL]) {
				const b = clickPlacedBox(ux, uy, f.w, f.h);
				expect(b.x).toBeGreaterThanOrEqual(0);
				expect(b.y).toBeGreaterThanOrEqual(0);
				expect(b.x + b.w).toBeLessThanOrEqual(1 + 1e-9);
				expect(b.y + b.h).toBeLessThanOrEqual(1 + 1e-9);
			}
		}
	});

	// A fixed UV height would read squat on 16:9 and stretched on a portrait
	// capture; the box is sized in visual pixels for that reason.
	it("holds the same screen ratio across frame shapes", () => {
		const wide = clickPlacedBox(0.5, 0.5, WIDE.w, WIDE.h);
		const tall = clickPlacedBox(0.5, 0.5, TALL.w, TALL.h);
		const ratio = (b: { w: number; h: number }, f: { w: number; h: number }) =>
			(b.w * f.w) / (b.h * f.h);
		expect(ratio(wide, WIDE)).toBeCloseTo(ratio(tall, TALL), 6);
	});
});

describe("clickPlacedArrow", () => {
	it("runs down-right from the click at a visual 45 degrees", () => {
		const a = clickPlacedArrow(0.2, 0.2, WIDE.w, WIDE.h);
		expect(a.x2).toBeGreaterThan(a.x1);
		expect(a.y2).toBeGreaterThan(a.y1);
		expect((a.x2 - a.x1) * WIDE.w).toBeCloseTo((a.y2 - a.y1) * WIDE.h, 6);
	});

	it("flips back toward the frame when clicked near the far corner", () => {
		const a = clickPlacedArrow(0.98, 0.98, WIDE.w, WIDE.h);
		expect(a.x2).toBeLessThan(a.x1);
		expect(a.y2).toBeLessThan(a.y1);
		expect(a.x2).toBeGreaterThanOrEqual(0);
		expect(a.y2).toBeGreaterThanOrEqual(0);
	});

	it("keeps both endpoints in frame from any click", () => {
		for (const [ux, uy] of [
			[0, 0],
			[1, 0],
			[0, 1],
			[1, 1],
			[0.5, 0.5],
		]) {
			for (const f of [WIDE, TALL]) {
				const a = clickPlacedArrow(ux, uy, f.w, f.h);
				for (const v of [a.x1, a.y1, a.x2, a.y2]) {
					expect(v).toBeGreaterThanOrEqual(0);
					expect(v).toBeLessThanOrEqual(1);
				}
			}
		}
	});
});
