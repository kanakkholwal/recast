import { describe, expect, it } from "vitest";
import { TITLE_PRESETS } from "./title-presets";

describe("title presets", () => {
	it("all build a sized, non-degenerate text annotation", () => {
		for (const p of TITLE_PRESETS) {
			const k = p.build();
			expect(k.kind).toBe("text");
			// Unlike the drag-to-place text tool (w/h = 0), presets are ready to show.
			expect(k.w).toBeGreaterThan(0);
			expect(k.h).toBeGreaterThan(0);
			expect(k.content.length).toBeGreaterThan(0);
			// Stays inside the frame.
			expect(k.x).toBeGreaterThanOrEqual(0);
			expect(k.y).toBeGreaterThanOrEqual(0);
			expect(k.x + k.w).toBeLessThanOrEqual(1);
			expect(k.y + k.h).toBeLessThanOrEqual(1);
			// Font size within the model's 0.02–0.20 range.
			expect(k.fontSize).toBeGreaterThanOrEqual(0.02);
			expect(k.fontSize).toBeLessThanOrEqual(0.2);
		}
	});

	it("builds fresh objects (no shared mutable ref between inserts)", () => {
		const a = TITLE_PRESETS[0].build();
		const b = TITLE_PRESETS[0].build();
		expect(a).not.toBe(b);
		a.content = "changed";
		expect(b.content).not.toBe("changed");
	});

	it("orders title larger than subtitle and puts the lower-third at the bottom", () => {
		const title = TITLE_PRESETS.find((p) => p.id === "title")!.build();
		const subtitle = TITLE_PRESETS.find((p) => p.id === "subtitle")!.build();
		const lower = TITLE_PRESETS.find((p) => p.id === "lower-third")!.build();
		expect(title.fontSize).toBeGreaterThan(subtitle.fontSize);
		expect(title.y).toBeLessThan(subtitle.y); // title sits above subtitle
		expect(lower.y).toBeGreaterThan(0.6); // lower third lives in the bottom band
		expect(lower.align).toBe("left");
	});
});
