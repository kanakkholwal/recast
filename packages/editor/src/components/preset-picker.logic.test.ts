import { describe, expect, it } from "vitest";
import {
	aspectClass,
	bgPreviewStyle,
	buildModel,
	clampIndex,
	filterPresets,
	frameInsetPct,
	groupPresets,
	resolveEscape,
	rowMoveIndex,
	score,
	wallpaperId,
} from "./preset-picker.logic";

const preset = {
	label: "Studio",
	category: "Studio",
	aspect: "16:9",
	description: "Soft blue gradient",
	keywords: ["clean", "demo"],
	bg: "gradient",
	value: "linear-gradient(#000,#fff)",
};

describe("score", () => {
	it("matches everything on an empty query", () => {
		expect(score(preset, "")).toBe(1);
	});
	it("ranks label prefix above substring and keyword", () => {
		expect(score(preset, "stu")).toBe(100);
		// label substring (90/category-prefix excluded by a non-matching category)
		expect(score({ ...preset, label: "My Studio", category: "Other" }, "studio")).toBe(80);
		expect(score({ ...preset, label: "X", category: "Y" }, "demo")).toBe(30);
	});
	it("returns 0 for no match", () => {
		expect(score({ ...preset, keywords: [], description: "" }, "zzz")).toBe(0);
	});
});

describe("frameInsetPct", () => {
	it("is 0 at no padding and capped at 20", () => {
		expect(frameInsetPct(0)).toBe(0);
		expect(frameInsetPct(10_000)).toBe(20);
	});
	it("mirrors video occupancy 1/(1+2p)", () => {
		// padding 50% → p=0.5 → 0.5/2 = 25% → capped to 20
		expect(frameInsetPct(50)).toBe(20);
		// padding 10% → p=0.1 → 0.1/1.2 ≈ 8.33%
		expect(frameInsetPct(10)).toBeCloseTo(8.333, 2);
	});
});

describe("aspectClass", () => {
	it("maps known aspects, defaults to video", () => {
		expect(aspectClass("1:1")).toBe("aspect-square");
		expect(aspectClass("9:16")).toBe("aspect-[9/16]");
		expect(aspectClass("weird")).toBe("aspect-video");
	});
});

describe("wallpaperId", () => {
	it("strips asset prefixes", () => {
		expect(wallpaperId({ ...preset, value: "asset://abc" })).toBe("abc");
		expect(wallpaperId({ ...preset, value: "asset:xyz" })).toBe("xyz");
	});
});

describe("bgPreviewStyle", () => {
	it("uses the value for gradient/color, muted otherwise", () => {
		expect(bgPreviewStyle(preset)).toBe("background:linear-gradient(#000,#fff)");
		expect(bgPreviewStyle({ ...preset, bg: "wallpaper" })).toBe("background:var(--color-muted)");
	});
});

// ── Navigation model ──
const P = (label: string, category: string) => ({
	label,
	category,
	aspect: "16:9",
});

describe("filterPresets", () => {
	const presets = [P("Studio", "Studio"), P("Reel", "Social"), P("Story", "Social")];
	it("returns all in order for an empty query", () => {
		expect(filterPresets(presets, "").map((p) => p.label)).toEqual(["Studio", "Reel", "Story"]);
	});
	it("filters and ranks by score", () => {
		expect(filterPresets(presets, "st").map((p) => p.label)).toEqual(["Studio", "Story"]);
	});
});

describe("groupPresets", () => {
	const filtered = [P("Studio", "Studio"), P("Reel", "Instagram")];
	it("flattens to a single Results group while searching", () => {
		const g = groupPresets(filtered, "x", null);
		expect(g).toHaveLength(1);
		expect(g[0][0]).toBe("Results");
	});
	it("groups by category and pins Current on top", () => {
		const current = filtered[0];
		const g = groupPresets(filtered, "", current);
		expect(g[0][0]).toBe("Current");
		expect(g[0][1]).toEqual([current]);
	});
	it("does not repeat the pinned preset inside its own category", () => {
		const current = filtered[0];
		const g = groupPresets(filtered, "", current);
		expect(g.map(([c]) => c)).toEqual(["Current", "Instagram"]);
	});
	it("keeps a category that still holds other presets", () => {
		const items = [P("Studio", "Studio"), P("Focus", "Studio")];
		const g = groupPresets(items, "", items[0]);
		expect(g.map(([c]) => c)).toEqual(["Current", "Studio"]);
		expect(g[1][1].map((x) => x.label)).toEqual(["Focus"]);
	});
});

describe("resolveEscape", () => {
	it("clears a typed query before it closes the dialog", () => {
		expect(resolveEscape("story")).toBe("clear");
	});
	it("closes on an empty or whitespace query", () => {
		expect(resolveEscape("")).toBe("close");
		expect(resolveEscape("   ")).toBe("close");
	});
});

describe("buildModel + rowMoveIndex", () => {
	// 3 presets in one category, 2 cols → rows [[0,1],[2]]
	const grouped: [string, ReturnType<typeof P>[]][] = [
		["Studio", [P("a", "Studio"), P("b", "Studio"), P("c", "Studio")]],
	];
	const model = buildModel(grouped, 2);

	it("chunks into rows with unique running indices", () => {
		expect(model.flat.map((p) => p.label)).toEqual(["a", "b", "c"]);
		expect(model.rows.map((r) => r.map((c) => c.index))).toEqual([[0, 1], [2]]);
	});
	it("moves down preserving column, clamped on a short row", () => {
		// from index 1 (row0 col1) down → row1 has only col0 → index 2
		expect(rowMoveIndex(model, 1, 1)).toBe(2);
		// from index 0 (row0 col0) down → row1 col0 → index 2
		expect(rowMoveIndex(model, 0, 1)).toBe(2);
	});
	it("returns null past the edges", () => {
		expect(rowMoveIndex(model, 0, -1)).toBeNull();
		expect(rowMoveIndex(model, 2, 1)).toBeNull();
	});
});

describe("clampIndex", () => {
	it("clamps into range", () => {
		expect(clampIndex(-2, 5)).toBe(0);
		expect(clampIndex(9, 5)).toBe(4);
		expect(clampIndex(3, 5)).toBe(3);
	});
});
