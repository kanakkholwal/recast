import { describe, expect, it } from "vitest";
import {
	boolParam,
	PANEL_PARAM,
	parseBoolParam,
	parsePanelTab,
	SIDEBAR_PARAM,
	TIMELINE_PARAM,
	withEditorParams,
} from "./editor-url";
import { PANEL_TABS } from "./panel-tabs";

const at = (search: string) => new URL(`recast://editor/clip.recast${search}`);

describe("parsePanelTab", () => {
	// Guards the drift the const array exists to prevent: a new store tab must work in the URL with no second edit.
	it("accepts every tab the store defines", () => {
		for (const tab of PANEL_TABS) {
			expect(parsePanelTab(tab, true)).toBe(tab);
		}
	});

	it("rejects unknown, empty and missing values", () => {
		expect(parsePanelTab("nope")).toBeNull();
		expect(parsePanelTab("")).toBeNull();
		expect(parsePanelTab(null)).toBeNull();
		expect(parsePanelTab(undefined)).toBeNull();
	});

	it("tolerates case and surrounding whitespace", () => {
		expect(parsePanelTab(" Captions ")).toBe("captions");
	});

	// A production URL naming `dev` would select a tab with no trigger in the rail, leaving nothing highlighted.
	it("gates the dev-only tab on the build", () => {
		expect(parsePanelTab("dev")).toBeNull();
		expect(parsePanelTab("dev", true)).toBe("dev");
	});
});

describe("parseBoolParam", () => {
	it("reads the forms a URL might carry", () => {
		for (const v of ["1", "true", "TRUE", " yes ", "on"]) expect(parseBoolParam(v)).toBe(true);
		for (const v of ["0", "false", "no", "off"]) expect(parseBoolParam(v)).toBe(false);
	});

	// Null, not false: an absent or junk param must leave the remembered preference alone, not collapse the panel.
	it("returns null when absent or unrecognised", () => {
		expect(parseBoolParam(null)).toBeNull();
		expect(parseBoolParam(undefined)).toBeNull();
		expect(parseBoolParam("")).toBeNull();
		expect(parseBoolParam("maybe")).toBeNull();
	});

	it("round-trips what boolParam writes", () => {
		expect(parseBoolParam(boolParam(true))).toBe(true);
		expect(parseBoolParam(boolParam(false))).toBe(false);
	});
});

describe("withEditorParams", () => {
	const full = { [PANEL_PARAM]: "audio", [SIDEBAR_PARAM]: "1", [TIMELINE_PARAM]: "0" };

	it("sets every param when none are present", () => {
		const next = withEditorParams(at(""), full);
		expect(next?.searchParams.get(PANEL_PARAM)).toBe("audio");
		expect(next?.searchParams.get(SIDEBAR_PARAM)).toBe("1");
		expect(next?.searchParams.get(TIMELINE_PARAM)).toBe("0");
	});

	// The whole reason this takes a map: a per-param writer would rebuild from a stale URL and drop sibling changes.
	it("applies every change in one pass when only one differs", () => {
		const next = withEditorParams(at("?tab=audio&sidebar=1&timeline=1"), full);
		expect(next?.searchParams.get(TIMELINE_PARAM)).toBe("0");
		expect(next?.searchParams.get(PANEL_PARAM)).toBe("audio");
		expect(next?.searchParams.get(SIDEBAR_PARAM)).toBe("1");
	});

	// Null is the loop-breaker: the writer must not touch history when the URL already agrees with the app.
	it("returns null when the URL already says all of them", () => {
		expect(withEditorParams(at("?tab=audio&sidebar=1&timeline=0"), full)).toBeNull();
	});

	it("leaves other params and the path alone", () => {
		const next = withEditorParams(at("?keep=1"), full);
		expect(next?.searchParams.get("keep")).toBe("1");
		expect(next?.pathname).toBe(at("").pathname);
	});

	it("does not mutate the URL it was given", () => {
		const url = at("?tab=info");
		withEditorParams(url, full);
		expect(url.searchParams.get(PANEL_PARAM)).toBe("info");
	});
});
