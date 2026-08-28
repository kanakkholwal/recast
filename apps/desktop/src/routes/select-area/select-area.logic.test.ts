import { describe, expect, it } from "vitest";
import {
	clampToolbar,
	confirmLabel,
	hintLabel,
	overlayMode,
	rectFromPoints,
	TOOLBAR_H,
	TOOLBAR_W,
	toRegionPayload,
} from "./select-area.logic";

describe("overlayMode", () => {
	it("reads the screenshot mode off the query string", () => {
		expect(overlayMode("?mode=screenshot")).toBe("screenshot");
	});

	// A spawned overlay with no mode is the recorder's, which predates the flag.
	it("defaults to recording", () => {
		expect(overlayMode("")).toBe("record");
		expect(overlayMode("?mode=")).toBe("record");
		expect(overlayMode("?mode=nonsense")).toBe("record");
	});
});

describe("labels", () => {
	it("name the action each mode performs", () => {
		expect(confirmLabel("screenshot")).toBe("Capture");
		expect(confirmLabel("record")).toBe("Use area");
		expect(hintLabel("screenshot")).toContain("capture");
		expect(hintLabel("record")).toContain("select");
	});
});

describe("rectFromPoints", () => {
	it("normalises a drag that went up and to the left", () => {
		expect(rectFromPoints(100, 80, 40, 20)).toEqual({ x: 40, y: 20, w: 60, h: 60 });
	});
});

describe("toRegionPayload", () => {
	// Dropping the ratio is how a Retina selection captures a quarter of the area.
	it("shifts by the overlay origin and scales to physical pixels", () => {
		const payload = toRegionPayload({ x: 10, y: 20, w: 30, h: 40 }, { x: 100, y: 200 }, 2);
		expect(payload).toMatchObject({ x: 220, y: 440, width: 60, height: 80 });
	});

	it("labels the area in the physical pixels it will capture", () => {
		const payload = toRegionPayload({ x: 0, y: 0, w: 30, h: 40 }, { x: 0, y: 0 }, 2);
		expect(payload.label).toBe("Area 60×80");
	});
});

describe("clampToolbar", () => {
	it("sits under the selection when there is room", () => {
		expect(clampToolbar({ x: 50, y: 50, w: 100, h: 100 }, 1000, 1000)).toEqual({
			left: 50,
			top: 156,
		});
	});

	// A selection to the bottom edge would push the toolbar off screen.
	it("flips above the selection when it would overflow the bottom", () => {
		const { top } = clampToolbar({ x: 10, y: 900, w: 100, h: 90 }, 1000, 1000);
		expect(top).toBe(900 - TOOLBAR_H - 6);
	});

	it("keeps the toolbar on screen at the right edge", () => {
		const { left } = clampToolbar({ x: 980, y: 10, w: 10, h: 10 }, 1000, 1000);
		expect(left).toBe(1000 - TOOLBAR_W - 8);
	});
});
