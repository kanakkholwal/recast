import { describe, expect, it } from "vitest";
import { dotStyleFor, labelFor } from "./camera-panel.logic";

describe("dotStyleFor", () => {
	// The preset ids mix conventions ('top-left' = row-col, 'left-center' =
	// col-row), so these lock the token-based parsing against a regression.
	it("places corner presets by row and column", () => {
		expect(dotStyleFor("top-left")).toContain("left:18%;");
		expect(dotStyleFor("top-left")).toContain("top:18%;");
		expect(dotStyleFor("bottom-right")).toContain("right:18%;");
		expect(dotStyleFor("bottom-right")).toContain("bottom:18%;");
	});

	it("puts left-center on the left edge, vertically centred (not bottom)", () => {
		const s = dotStyleFor("left-center");
		expect(s).toContain("left:18%;");
		expect(s).toContain("top:50%;");
		expect(s).not.toContain("bottom:");
	});

	it("puts right-center on the right edge, vertically centred", () => {
		const s = dotStyleFor("right-center");
		expect(s).toContain("right:18%;");
		expect(s).toContain("top:50%;");
	});

	it("puts bottom-center at the bottom, horizontally centred", () => {
		const s = dotStyleFor("bottom-center");
		expect(s).toContain("bottom:18%;");
		expect(s).toContain("left:50%;");
	});

	it("puts top-center at the top, horizontally centred", () => {
		const s = dotStyleFor("top-center");
		expect(s).toContain("top:18%;");
		expect(s).toContain("left:50%;");
	});
});

describe("labelFor", () => {
	it("title-cases the preset id", () => {
		expect(labelFor("left-center")).toBe("Left Center");
		expect(labelFor("bottom-right")).toBe("Bottom Right");
	});
});
