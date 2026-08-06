import { describe, expect, it } from "vitest";
import { parsePanelTab } from "./editor-url";
import { PANEL_TABS, WEB_PANEL_TABS } from "./panel-tabs";
import {
	type EditorServices,
	getEditorServices,
	setEditorServicesForApp,
	tryGetEditorServices,
} from "./services";

const minimal: EditorServices = { resolveAssetUrl: (r) => r };

describe("EditorServices registry", () => {
	it("throws only when nothing was ever installed", () => {
		const restore = setEditorServicesForApp(minimal);
		expect(getEditorServices()).toBe(minimal);
		restore();
	});

	it("restores the previous services, so one editor cannot leak into the next", () => {
		const first: EditorServices = { resolveAssetUrl: (r) => `first:${r}` };
		const undoFirst = setEditorServicesForApp(first);
		const undoSecond = setEditorServicesForApp(minimal);
		expect(getEditorServices()).toBe(minimal);
		undoSecond();
		expect(getEditorServices()).toBe(first);
		undoFirst();
	});

	// The whole contract: an omitted capability reads as absent rather than
	// throwing when a panel calls it.
	it("reports omitted capabilities as undefined", () => {
		const restore = setEditorServicesForApp(minimal);
		const s = getEditorServices();
		expect(s.transcription).toBeUndefined();
		expect(s.analysis).toBeUndefined();
		expect(s.mediaAnalysis).toBeUndefined();
		expect(s.shell).toBeUndefined();
		expect(s.ocr).toBeUndefined();
		expect(s.pickFile).toBeUndefined();
		restore();
	});

	it("never throws from tryGetEditorServices outside a component", () => {
		expect(() => tryGetEditorServices()).not.toThrow();
	});
});

describe("WEB_PANEL_TABS", () => {
	it("is a subset of the full tab list", () => {
		for (const tab of WEB_PANEL_TABS) expect(PANEL_TABS).toContain(tab);
	});

	it("omits the tabs that need a native host", () => {
		expect(WEB_PANEL_TABS).not.toContain("cursor");
		expect(WEB_PANEL_TABS).not.toContain("music");
		expect(WEB_PANEL_TABS).not.toContain("dev");
	});

	// A deep link to a tab the host doesn't serve must fall back, not render a
	// dead panel.
	it("rejects an unavailable tab when parsing a URL param", () => {
		expect(parsePanelTab("cursor", false, WEB_PANEL_TABS)).toBeNull();
		expect(parsePanelTab("cursor")).toBe("cursor");
		expect(parsePanelTab("captions", false, WEB_PANEL_TABS)).toBe("captions");
	});
});
