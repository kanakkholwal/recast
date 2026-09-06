import { describe, expect, it } from "vitest";
import {
	DEFAULT_SETTINGS_TAB,
	parseSettingsTab,
	SETTINGS_TABS,
	settingsHref,
} from "./settings-tabs";

describe("parseSettingsTab", () => {
	// Guards the drift the const array exists to prevent: a new tab must work in the URL with no second edit.
	it("accepts every tab the page defines", () => {
		for (const tab of SETTINGS_TABS) {
			expect(parseSettingsTab(tab)).toBe(tab);
		}
	});

	it("tolerates case and surrounding whitespace", () => {
		expect(parseSettingsTab("  Cloud ")).toBe("cloud");
	});

	// Null, not the default: the caller decides what no-tab means, and junk must not look like an explicit choice.
	it("returns null for unknown, empty and missing values", () => {
		expect(parseSettingsTab("nope")).toBeNull();
		expect(parseSettingsTab("")).toBeNull();
		expect(parseSettingsTab(null)).toBeNull();
		expect(parseSettingsTab(undefined)).toBeNull();
	});
});

describe("settingsHref", () => {
	it("round-trips through the parser", () => {
		for (const tab of SETTINGS_TABS) {
			const url = new URL(settingsHref(tab), "recast://app");
			expect(parseSettingsTab(url.searchParams.get("tab"))).toBe(tab);
		}
	});

	it("points at the settings route", () => {
		expect(settingsHref("cloud").startsWith("/settings?")).toBe(true);
	});
});

describe("DEFAULT_SETTINGS_TAB", () => {
	it("is a real tab", () => {
		expect(SETTINGS_TABS).toContain(DEFAULT_SETTINGS_TAB);
	});
});
