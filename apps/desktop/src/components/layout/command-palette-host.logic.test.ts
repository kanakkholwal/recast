import { describe, expect, it } from "vitest";
import type { PaletteCommand } from "$lib/stores/command-palette.svelte";
import {
	groupCommands,
	highlight,
	matchScore,
	rankCommands,
} from "./command-palette-host.logic";

function cmd(over: Partial<PaletteCommand> = {}): PaletteCommand {
	return {
		id: over.title ?? "c",
		title: "Export video",
		category: "Export",
		action: () => {},
		...over,
	};
}

describe("matchScore", () => {
	it("scores every command with an empty query", () => {
		expect(matchScore(cmd(), "")).toBe(1);
	});

	it("ranks title prefix > substring > description > keywords > category", () => {
		const c = cmd({
			title: "Export video",
			description: "Save as MP4",
			keywords: ["render"],
			category: "Output",
		});
		expect(matchScore(c, "exp")).toBe(100); // title prefix
		expect(matchScore(c, "video")).toBe(80); // title substring
		expect(matchScore(c, "mp4")).toBe(60); // description
		expect(matchScore(c, "render")).toBe(40); // keyword
		expect(matchScore(c, "output")).toBe(20); // category
		expect(matchScore(c, "zzz")).toBe(0); // no match
	});
});

describe("rankCommands", () => {
	it("drops non-matches and orders by descending score", () => {
		// Neutral categories so the only "export" hits come from the titles.
		const list = [
			cmd({ title: "Save project", category: "General" }),
			cmd({ title: "Export video", category: "General" }),
			cmd({ title: "My export shortcut", category: "General" }), // contains, lower than prefix
		];
		const ranked = rankCommands(list, "export");
		expect(ranked.map((c) => c.title)).toEqual([
			"Export video",
			"My export shortcut",
		]);
	});
});

describe("groupCommands", () => {
	it("groups by category when the query is empty", () => {
		const list = [
			cmd({ title: "A", category: "Edit" }),
			cmd({ title: "B", category: "Export" }),
			cmd({ title: "C", category: "Edit" }),
		];
		const groups = groupCommands(list, "");
		expect(groups.map(([cat]) => cat)).toEqual(["Edit", "Export"]);
		expect(groups[0][1]).toHaveLength(2);
	});

	it("collapses to a single Results group when searching", () => {
		const list = [cmd({ title: "A" }), cmd({ title: "B" })];
		const groups = groupCommands(list, "a");
		expect(groups).toHaveLength(1);
		expect(groups[0][0]).toBe("Results");
	});
});

describe("highlight", () => {
	it("returns one un-highlighted run for a blank search", () => {
		expect(highlight("Export", "  ")).toEqual([{ text: "Export", hl: false }]);
	});

	it("flags matching runs case-insensitively", () => {
		expect(highlight("Export video", "video")).toEqual([
			{ text: "Export ", hl: false },
			{ text: "video", hl: true },
		]);
	});

	it("escapes regex metacharacters in the search", () => {
		expect(highlight("a.b", ".")).toEqual([
			{ text: "a", hl: false },
			{ text: ".", hl: true },
			{ text: "b", hl: false },
		]);
	});
});
