import { describe, expect, it } from "vitest";
import { TimelinePointer } from "./canvas-pointer";

describe("TimelinePointer", () => {
	it("tests the pointer against regions drawn the PREVIOUS frame", () => {
		const p = new TimelinePointer();
		p.set(50, 10);
		// Frame 1: record a region, but nothing was drawn last frame yet.
		p.region({ id: "a", x: 0, y: 0, w: 100, h: 20 });
		expect(p.hit()).toBeNull();
		p.reset();
		// Frame 2: last frame's region is now testable.
		expect(p.hit()?.id).toBe("a");
	});

	it("returns the topmost (last-drawn) region under the pointer", () => {
		const p = new TimelinePointer();
		p.region({ id: "under", x: 0, y: 0, w: 100, h: 100 });
		p.region({ id: "over", x: 40, y: 40, w: 20, h: 20 });
		p.reset();
		p.set(50, 50);
		expect(p.hit()?.id).toBe("over");
		p.set(10, 10);
		expect(p.hit()?.id).toBe("under");
	});

	it("misses when the pointer is outside every region", () => {
		const p = new TimelinePointer();
		p.region({ id: "a", x: 0, y: 0, w: 10, h: 10 });
		p.reset();
		p.set(999, 999);
		expect(p.hit()).toBeNull();
		p.clear();
		expect(p.hit()).toBeNull();
	});

	it("resets the claimed cursor each frame", () => {
		const p = new TimelinePointer();
		p.cursor = "ew-resize";
		p.reset();
		expect(p.cursor).toBe("default");
	});
});
