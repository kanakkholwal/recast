import { describe, expect, it } from "vitest";
import { cameraAvailability, dotStyleFor, labelFor } from "./camera-panel.logic";

describe("cameraAvailability", () => {
	it("enables the overlay only when a separate track resolved to a file", () => {
		expect(cameraAvailability("separate", true).editable).toBe(true);
		expect(cameraAvailability("separate", false).editable).toBe(false);
		expect(cameraAvailability("off", false).editable).toBe(false);
		expect(cameraAvailability("failed", false).editable).toBe(false);
		expect(cameraAvailability("legacy", false).editable).toBe(false);
	});

	// The camera WAS switched on; blaming the user for not enabling it is the
	// exact wrong thing to say, and a warning already fired at record time.
	it("does not tell a failed capture to turn the camera on", () => {
		const failed = cameraAvailability("failed", false);
		expect(failed.description).not.toMatch(/turn the camera on/i);
		expect(failed.description).toMatch(/in use by another app|permission/i);
		expect(failed.title).not.toMatch(/no camera/i);
	});

	// The whole reason capture state is carried separately from the file path:
	// "you left the camera off" is wrong for a project recorded before the
	// camera could be captured at all.
	it("distinguishes a camera that was off from a project that predates capture", () => {
		const off = cameraAvailability("off", false).description;
		const legacy = cameraAvailability("legacy", false).description;
		expect(off).not.toBe(legacy);
		expect(off).toMatch(/turn the camera on/i);
		expect(legacy).toMatch(/predates/i);
	});

	// A recorded-but-missing track is a broken project, not a choice, and must
	// never read as "no camera was recorded".
	it("calls out a recorded track whose file is missing", () => {
		const missing = cameraAvailability("separate", false);
		expect(missing.title).toMatch(/missing/i);
		expect(missing.description).not.toMatch(/no camera was recorded/i);
	});

	// Every unavailable state must read differently; two identical messages mean
	// one of them is guessing.
	it("gives each unavailable state its own wording", () => {
		const descriptions = (["off", "failed", "legacy"] as const).map(
			(c) => cameraAvailability(c, false).description,
		);
		expect(new Set(descriptions).size).toBe(descriptions.length);
	});

	it("never returns empty copy for any state", () => {
		for (const capture of ["separate", "off", "failed", "legacy"] as const) {
			for (const hasFile of [true, false]) {
				const result = cameraAvailability(capture, hasFile);
				expect(result.title.length, `${capture}/${hasFile}`).toBeGreaterThan(0);
				expect(result.description.length, `${capture}/${hasFile}`).toBeGreaterThan(0);
			}
		}
	});
});

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
