import { describe, expect, it } from "vitest";
import type { FieldChange } from "../lib/agent/branches";
import {
	fieldLabel,
	formatValue,
	groupLabel,
	relativeAge,
	summariseChanges,
	toRow,
	toSections,
} from "./branch-review.logic";

function change(field: string, before: unknown, after: unknown): FieldChange {
	return { field, before, after };
}

describe("groupLabel", () => {
	it("uses the friendly name for a known collection", () => {
		expect(groupLabel("zoomRegions")).toBe("Zoom");
	});

	it("humanises an unknown key", () => {
		expect(groupLabel("borderRadius")).toBe("Border radius");
	});
});

describe("fieldLabel", () => {
	it("humanises a bare scalar", () => {
		expect(fieldLabel("trimStart")).toBe("Trim start");
	});

	it("numbers an indexed row from one", () => {
		expect(fieldLabel("cuts.0")).toBe("Cut 1");
	});

	it("appends the leaf under an indexed row", () => {
		expect(fieldLabel("cuts.2.end")).toBe("Cut 3 end");
	});

	it("reads a nested settings path", () => {
		expect(fieldLabel("audioSettings.volume")).toBe("Volume");
	});
});

describe("formatValue", () => {
	it("renders a missing side as a dash", () => {
		expect(formatValue(null)).toBe("—");
	});

	it("renders a boolean as on or off", () => {
		expect(formatValue(false)).toBe("off");
	});

	it("keeps an integer bare", () => {
		expect(formatValue(4)).toBe("4");
	});

	it("trims trailing zeros off a float", () => {
		expect(formatValue(1.5)).toBe("1.5");
	});

	it("rounds a long float to two places", () => {
		expect(formatValue(1.23456)).toBe("1.23");
	});

	it("counts a long array rather than listing it", () => {
		expect(formatValue([1, 2, 3, 4])).toBe("4 items");
	});

	it("lists a short array inline", () => {
		expect(formatValue([1, 2])).toBe("1, 2");
	});

	it("calls an empty array empty", () => {
		expect(formatValue([])).toBe("empty");
	});

	it("counts a wide object rather than listing it", () => {
		expect(formatValue({ a: 1, b: 2, c: 3, d: 4 })).toBe("4 fields");
	});

	it("lists a narrow object inline", () => {
		expect(formatValue({ start: 1, end: 2 })).toBe("start 1, end 2");
	});

	it("truncates a long string", () => {
		expect(formatValue("x".repeat(80))).toHaveLength(48);
	});
});

describe("toRow", () => {
	it("labels an added row", () => {
		expect(toRow(change("cuts.0", null, { start: 1 })).kind).toBe("added");
	});

	it("formats both sides", () => {
		const row = toRow(change("trimStart", 1, 2.5));

		expect([row.before, row.after]).toEqual(["1", "2.5"]);
	});
});

describe("toSections", () => {
	it("keeps first-appearance order", () => {
		const sections = toSections([
			change("trimStart", 0, 1),
			change("cuts.0.end", 1, 2),
			change("trimEnd", 9, 8),
		]);

		expect(sections.map((section) => section.group)).toEqual(["trimStart", "cuts", "trimEnd"]);
	});

	it("collects every row of a group together", () => {
		const sections = toSections([
			change("cuts.0.end", 1, 2),
			change("trimStart", 0, 1),
			change("cuts.1.start", 3, 4),
		]);

		expect(sections[0].rows).toHaveLength(2);
	});

	it("returns nothing for no changes", () => {
		expect(toSections([])).toEqual([]);
	});
});

describe("summariseChanges", () => {
	it("says so when a branch changes nothing", () => {
		expect(summariseChanges([])).toBe("No changes");
	});

	it("uses the singular for one change", () => {
		expect(summariseChanges([change("cuts.0", null, {})])).toBe("1 change in Cuts");
	});

	it("joins two groups with and", () => {
		const summary = summariseChanges([change("cuts.0", null, {}), change("trimStart", 0, 1)]);

		expect(summary).toBe("2 changes in Cuts and Trim start");
	});

	it("counts the overflow past two groups", () => {
		const summary = summariseChanges([
			change("cuts.0", null, {}),
			change("trimStart", 0, 1),
			change("zoomRegions.0", null, {}),
			change("audioSettings.volume", 1, 2),
		]);

		expect(summary).toBe("4 changes in Cuts, Trim start +2");
	});
});

describe("relativeAge", () => {
	const now = 1_700_000_000_000;

	it("calls the last minute just now", () => {
		expect(relativeAge(now - 30_000, now)).toBe("just now");
	});

	it("counts minutes", () => {
		expect(relativeAge(now - 5 * 60_000, now)).toBe("5m ago");
	});

	it("counts hours", () => {
		expect(relativeAge(now - 3 * 3_600_000, now)).toBe("3h ago");
	});

	it("counts days", () => {
		expect(relativeAge(now - 2 * 86_400_000, now)).toBe("2d ago");
	});

	it("never reports a negative age", () => {
		expect(relativeAge(now + 10_000, now)).toBe("just now");
	});
});
