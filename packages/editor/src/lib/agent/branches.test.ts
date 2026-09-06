import { describe, expect, it } from "vitest";
import {
	changeGroup,
	describeChange,
	EDIT_OP_TAGS,
	type FieldChange,
	groupChanges,
} from "./branches";

function change(field: string, before: unknown, after: unknown): FieldChange {
	return { field, before, after };
}

describe("EDIT_OP_TAGS", () => {
	it("matches the tag list ops.rs asserts", () => {
		expect([...EDIT_OP_TAGS]).toEqual([
			"replace",
			"trim",
			"cutAdd",
			"cutRemove",
			"zoomAdd",
			"zoomRemove",
			"splitPointAdd",
			"splitPointRemove",
			"speedSet",
			"speedRemove",
			"annotationAdd",
			"annotationUpdate",
			"annotationRemove",
			"animationAdd",
			"animationRemove",
			"set",
		]);
	});

	it("has no duplicates", () => {
		expect(new Set(EDIT_OP_TAGS).size).toBe(EDIT_OP_TAGS.length);
	});
});

describe("changeGroup", () => {
	it("takes the collection off an indexed path", () => {
		expect(changeGroup("cuts.0.end")).toBe("cuts");
	});

	it("returns a bare scalar unchanged", () => {
		expect(changeGroup("trimStart")).toBe("trimStart");
	});
});

describe("describeChange", () => {
	it("calls a missing before an addition", () => {
		expect(describeChange(change("cuts.0", null, { start: 1 }))).toBe("added");
	});

	it("calls a missing after a removal", () => {
		expect(describeChange(change("cuts.0", { start: 1 }, null))).toBe("removed");
	});

	it("calls two present sides a change", () => {
		expect(describeChange(change("trimStart", 1, 2))).toBe("changed");
	});

	it("treats undefined the same as null", () => {
		expect(describeChange(change("cuts.0", undefined, { start: 1 }))).toBe("added");
	});
});

describe("groupChanges", () => {
	it("buckets by the top-level collection", () => {
		const groups = groupChanges([
			change("cuts.0.end", 1, 2),
			change("trimStart", 0, 1),
			change("cuts.1.start", 3, 4),
		]);

		expect([...groups.keys()]).toEqual(["cuts", "trimStart"]);
	});

	it("keeps path order inside a bucket", () => {
		const groups = groupChanges([
			change("cuts.0.end", 1, 2),
			change("trimStart", 0, 1),
			change("cuts.1.start", 3, 4),
		]);

		expect(groups.get("cuts")?.map((c) => c.field)).toEqual(["cuts.0.end", "cuts.1.start"]);
	});

	it("returns nothing for an empty list", () => {
		expect(groupChanges([]).size).toBe(0);
	});
});
