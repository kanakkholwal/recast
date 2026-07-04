import { describe, expect, it } from "vitest";
import {
	arrowGeometry,
	blurTint,
	strokeDashPattern,
} from "./annotation-draw.logic";

describe("strokeDashPattern", () => {
	it("scales dash arrays by stroke width; solid is empty", () => {
		expect(strokeDashPattern("solid", 2)).toEqual([]);
		expect(strokeDashPattern(undefined, 2)).toEqual([]);
		expect(strokeDashPattern("dashed", 2)).toEqual([16, 12]);
		expect(strokeDashPattern("dotted", 3)).toEqual([6, 12]);
	});
});

describe("blurTint", () => {
	// alpha = (0.15 + 0.8*strength) * opacity, mirroring the export.
	it("scales white/black alpha by strength", () => {
		expect(blurTint("white", "", 1)).toBe("rgba(255,255,255,0.950)");
		expect(blurTint("black", "", 0.5)).toBe("rgba(0,0,0,0.550)");
	});
	it("multiplies by master opacity", () => {
		expect(blurTint("white", "", 1, 0.5)).toBe("rgba(255,255,255,0.475)");
	});
	it("parses a #rrggbb colour variant at the strength alpha", () => {
		expect(blurTint("color", "#ff8000", 1)).toBe("rgba(255,128,0,0.950)");
		expect(blurTint("color", "00ff00", 0)).toBe("rgba(0,255,0,0.150)");
	});
	it("glass has no tint until strength passes 0.6, then a grey wash", () => {
		expect(blurTint("glass", "", 0.5)).toBeNull();
		expect(blurTint("glass", "", 1)).toBe("rgba(128,128,128,0.240)");
	});
	it("returns null for invalid colour", () => {
		expect(blurTint("color", "nope", 1)).toBeNull();
	});
});

describe("arrowGeometry", () => {
	it("is null for a degenerate arrow", () => {
		expect(arrowGeometry({ x: 0, y: 0 }, { x: 0.5, y: 0 }, 2, 0.2)).toBeNull();
	});
	it("computes shaft end and symmetric head corners", () => {
		const g = arrowGeometry({ x: 0, y: 0 }, { x: 100, y: 0 }, 2, 0.2)!;
		// len 100, headLen max(4, 20)=20, headWidth 14
		expect(g.tip).toEqual({ x: 100, y: 0 });
		expect(g.lineEnd.x).toBeCloseTo(80, 5);
		expect(g.left.y).toBeCloseTo(7, 5);
		expect(g.right.y).toBeCloseTo(-7, 5);
	});
	it("respects the stroke-width floor on head length", () => {
		// tiny headSize → headLen floored at strokePx*2 = 20
		const g = arrowGeometry({ x: 0, y: 0 }, { x: 100, y: 0 }, 10, 0.01)!;
		expect(g.lineEnd.x).toBeCloseTo(80, 5);
	});
});
