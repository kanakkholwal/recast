import { describe, expect, it } from "vitest";
import {
	DEFAULT_LAYOUT,
	endOfClipAction,
	parseLayout,
	shouldEchoElementTime,
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
