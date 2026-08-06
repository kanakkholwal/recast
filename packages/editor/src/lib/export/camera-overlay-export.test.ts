import { describe, expect, it } from "vitest";
import type { FrameGeometry } from "../../components/frame-params";
import { bubbleCornerRadiusPx, cameraBubbleRect, coverUvRect } from "./camera-overlay-export";

// 1920x1080 video at (40,60) inside a 2000x1200 canvas — mirrors camera.rs tests.
const geom: FrameGeometry = {
	canvasW: 2000,
	canvasH: 1200,
	videoX: 40,
	videoY: 60,
	videoW: 1920,
	videoH: 1080,
};

const place = (x: number, y: number, width: number) => ({ x, y, width, height: width });

describe("cameraBubbleRect", () => {
	it("is square and sized off video width (like the Rust export)", () => {
		const r = cameraBubbleRect(place(0, 0, 0.2), geom, 2000, 1200);
		expect(r.w).toBeCloseTo(384); // 0.2 * 1920
		expect(r.h).toBeCloseTo(384);
	});

	it("clamps an out-of-canvas placement back inside", () => {
		const r = cameraBubbleRect(place(0.8, 0.8, 0.2), geom, 2000, 1200);
		expect(r.x).toBeCloseTo(1576); // 40 + 0.8*1920, within max
		expect(r.y).toBeCloseTo(816); // 60 + 0.8*1080 = 924 → clamped to 1200-384
	});

	it("scales comp-space px into a shrunk render buffer", () => {
		const r = cameraBubbleRect(place(0, 0, 0.2), geom, 1000, 600); // sx=sy=0.5
		expect(r.w).toBeCloseTo(192);
	});
});

describe("coverUvRect", () => {
	it("crops the width for a landscape source", () => {
		const uv = coverUvRect(16 / 9, false);
		expect(uv.du).toBeCloseTo(9 / 16);
		expect(uv.u0).toBeCloseTo((1 - 9 / 16) / 2);
		expect(uv.dv).toBe(1);
	});

	it("crops the height for a portrait source", () => {
		const uv = coverUvRect(9 / 16, false);
		expect(uv.dv).toBeCloseTo(9 / 16);
		expect(uv.du).toBe(1);
	});

	it("mirrors by flipping the u extent", () => {
		const plain = coverUvRect(16 / 9, false);
		const mirrored = coverUvRect(16 / 9, true);
		expect(mirrored.du).toBeCloseTo(-plain.du);
		expect(mirrored.u0).toBeCloseTo(plain.u0 + plain.du); // samples the far edge first
	});
});

describe("bubbleCornerRadiusPx", () => {
	it("circle → half the side (true circle on the square)", () => {
		expect(bubbleCornerRadiusPx("circle", undefined, 400)).toBe(200);
	});
	it("square/rectangle → sharp corners", () => {
		expect(bubbleCornerRadiusPx("square", 0.16, 400)).toBe(0);
		expect(bubbleCornerRadiusPx("rectangle", 0.16, 400)).toBe(0);
	});
	it("rounded → cornerRadius fraction of the side (default 0.16)", () => {
		expect(bubbleCornerRadiusPx("rounded", 0.25, 400)).toBe(100);
		expect(bubbleCornerRadiusPx("rounded", undefined, 400)).toBeCloseTo(64);
	});
});
