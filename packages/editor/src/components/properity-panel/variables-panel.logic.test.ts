import { describe, expect, it } from "vitest";
import type { VarSpec } from "../../lib/editor/render-state";
import {
	decimalsFor,
	groupVariables,
	readVec2,
	selectOptions,
	trimNumber,
	writeVec2,
} from "./variables-panel.logic";

function spec(over: Partial<VarSpec> = {}): VarSpec {
	return { name: "v", type: "number", value: "1", ...over };
}

describe("groupVariables", () => {
	it("leads with the ungrouped ones and keeps document order inside a group", () => {
		const groups = groupVariables([
			spec({ name: "a", path: "Brand" }),
			spec({ name: "b" }),
			spec({ name: "c", path: "Brand" }),
			spec({ name: "d", path: "Motion" }),
		]);

		expect(groups.map((g) => g.path)).toEqual(["", "Brand", "Motion"]);
		expect(groups[1].vars.map((v) => v.name)).toEqual(["a", "c"]);
	});

	it("treats a blank path as no path, so whitespace does not open a group", () => {
		const groups = groupVariables([spec({ name: "a", path: "  " }), spec({ name: "b" })]);

		expect(groups).toHaveLength(1);
		expect(groups[0].path).toBe("");
	});
});

describe("vec2", () => {
	it("round-trips through the spelling the document uses", () => {
		expect(readVec2("0.25,0.75")).toEqual([0.25, 0.75]);
		expect(writeVec2(0.25, 0.75)).toBe("0.25,0.75");
	});

	it("reads a malformed pair as the origin rather than NaN", () => {
		expect(readVec2("")).toEqual([0, 0]);
		expect(readVec2("wide")).toEqual([0, 0]);
		expect(readVec2("0.5")).toEqual([0.5, 0]);
	});
});

describe("trimNumber", () => {
	it("drops the trailing zeros a float would otherwise carry into the file", () => {
		expect(trimNumber(2)).toBe("2");
		expect(trimNumber(0.1 + 0.2)).toBe("0.3");
		expect(trimNumber(Number.NaN)).toBe("0");
	});
});

describe("decimalsFor", () => {
	it("shows none for an int or a whole step, and follows a fractional one", () => {
		expect(decimalsFor(spec({ type: "int" }))).toBe(0);
		expect(decimalsFor(spec({ step: 5 }))).toBe(0);
		expect(decimalsFor(spec({ step: 0.05 }))).toBe(2);
	});

	it("caps at three so a field cannot outgrow its column", () => {
		expect(decimalsFor(spec({ step: 0.0000001 }))).toBe(3);
	});
});

describe("selectOptions", () => {
	it("keeps a value the declaration forgot to list, rather than showing it as unset", () => {
		const options = selectOptions(spec({ type: "select", value: "teal", options: ["warm"] }));

		expect(options.map((o) => o.value)).toEqual(["warm", "teal"]);
	});

	it("does not duplicate a value that is already an option", () => {
		const options = selectOptions(
			spec({ type: "select", value: "warm", options: ["warm", "cool"] }),
		);

		expect(options.map((o) => o.value)).toEqual(["warm", "cool"]);
	});
});
