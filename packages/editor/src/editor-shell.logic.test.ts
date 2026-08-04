import { describe, expect, it } from "vitest";
import {
	clampSidebarWidth,
	DEFAULT_LAYOUT,
	endOfClipAction,
	parseLayout,
	shouldEchoElementTime,
	SIDEBAR_DEFAULT,
	SIDEBAR_MAX,
	SIDEBAR_MIN,
	visiblePanels,
} from "./editor-shell.logic";

describe("parseLayout", () => {
	it("round-trips a stored layout", () => {
		expect(parseLayout(JSON.stringify({ sidebar: false, timeline: true }))).toEqual({
			sidebar: false,
			timeline: true,
		});
	});

	// A corrupt or partial value must not hide the panels with no way back.
	it("falls back to everything visible", () => {
		expect(parseLayout(null)).toEqual(DEFAULT_LAYOUT);
		expect(parseLayout("not json")).toEqual(DEFAULT_LAYOUT);
		expect(parseLayout("{}")).toEqual(DEFAULT_LAYOUT);
		expect(parseLayout('{"sidebar":"yes"}')).toEqual(DEFAULT_LAYOUT);
	});
});

describe("clampSidebarWidth", () => {
	it("holds the panel between its bounds", () => {
		expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN);
		expect(clampSidebarWidth(99999)).toBe(SIDEBAR_MAX);
		expect(clampSidebarWidth(400.6)).toBe(401);
	});

	// localStorage returns "" for a missing key, and Number("") is 0 — which must
	// not collapse the panel to its minimum on first open.
	it("uses the default for a non-numeric stored value", () => {
		expect(clampSidebarWidth(Number.NaN)).toBe(SIDEBAR_DEFAULT);
	});
});

describe("transport", () => {
	it("loops only when looping is on", () => {
		expect(endOfClipAction(true)).toBe("loop");
		expect(endOfClipAction(false)).toBe("pause");
	});

	// Echoing the element's time while the WebCodecs clock owns playback snaps
	// the playhead back across every cut.
	it("ignores the element's time while WebCodecs drives the picture", () => {
		expect(shouldEchoElementTime(true)).toBe(false);
		expect(shouldEchoElementTime(false)).toBe(true);
	});
});

describe("visiblePanels", () => {
	it("keeps a panel unless it is explicitly unavailable", () => {
		expect(visiblePanels(["clip", "captions", "cursor"], { cursor: false })).toEqual([
			"clip",
			"captions",
		]);
		expect(visiblePanels(["clip", "captions"], {})).toEqual(["clip", "captions"]);
	});
});
