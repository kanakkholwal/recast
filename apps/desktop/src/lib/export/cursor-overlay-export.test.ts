import { describe, expect, it } from "vitest";
import type { SvgCursorParams } from "../../components/editor/frame-params";
import { cursorSpriteRect, pickCursorState } from "./cursor-overlay-export";

const base: SvgCursorParams = {
	visible: true,
	alpha: 1,
	styleId: "macos" as SvgCursorParams["styleId"],
	pressed: false,
	right: false,
	dragging: false,
	scale: 1,
	canvasX: 100,
	canvasY: 50,
	compW: 1000,
	compH: 500,
	spritePx: 32,
};

describe("pickCursorState", () => {
	it("maps press/right/drag flags to the sprite state like the preview", () => {
		expect(pickCursorState(base)).toBe("rest");
		expect(pickCursorState({ ...base, pressed: true })).toBe("press");
		expect(pickCursorState({ ...base, pressed: true, right: true })).toBe("rightPress");
		expect(pickCursorState({ ...base, pressed: true, dragging: true })).toBe("drag");
		// drag wins over right when both set (matches the DOM ternary order)
		expect(pickCursorState({ ...base, pressed: true, right: true, dragging: true })).toBe("drag");
	});
});

describe("cursorSpriteRect", () => {
	it("anchors the hotspot at the cursor sample point", () => {
		const r = cursorSpriteRect(base, [0.25, 0.5], 1000, 500);
		expect(r.w).toBe(32);
		expect(r.h).toBe(32);
		// hotspot pixel = rect origin + hot*size must equal (canvasX, canvasY)
		expect(r.x + 0.25 * r.w).toBeCloseTo(100);
		expect(r.y + 0.5 * r.h).toBeCloseTo(50);
	});

	it("scales about the hotspot (anchor stays put)", () => {
		const r = cursorSpriteRect({ ...base, scale: 2 }, [0.25, 0.5], 1000, 500);
		expect(r.w).toBe(64);
		expect(r.x + 0.25 * r.w).toBeCloseTo(100);
		expect(r.y + 0.5 * r.h).toBeCloseTo(50);
	});

	it("maps comp-space coords to a shrunk render buffer", () => {
		// canvas buffer half the comp size → sx=sy=0.5
		const r = cursorSpriteRect(base, [0, 0], 500, 250);
		expect(r.w).toBe(16);
		expect(r.x).toBeCloseTo(50);
		expect(r.y).toBeCloseTo(25);
	});
});
