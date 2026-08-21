import { describe, expect, it } from "vitest";
import {
	entrypointLabel,
	groupByDomain,
	neighbours,
	sortDocs,
	toArchitectureMeta,
} from "./meta.logic";
import type { ArchitectureMeta } from "./types";

function meta(over: Partial<ArchitectureMeta> = {}): ArchitectureMeta {
	return {
		slug: "a",
		url: "/architecture/a",
		title: "A",
		description: "",
		summary: "",
		position: 0,
		status: "production",
		domain: "platform",
		inputs: [],
		outputs: [],
		entrypoints: [],
		invariants: [],
		headings: [],
		...over,
	};
}

describe("toArchitectureMeta", () => {
	const frontmatter = {
		title: "Timeline model",
		description: "The arithmetic",
		summary: "Editing never touches media",
		position: 5,
		status: "production",
		domain: "editor",
		inputs: ["Trim bounds"],
		outputs: ["Kept segments"],
		entrypoints: ["packages/editor/src/lib/timeline/time-map.ts"],
		invariants: ["Output-axis first"],
	};

	it("carries the structured facts through", () => {
		const result = toArchitectureMeta(
			"timeline-model",
			"/architecture/timeline-model",
			frontmatter,
		);

		expect([result.domain, result.position, result.invariants]).toEqual([
			"editor",
			5,
			["Output-axis first"],
		]);
	});

	it("falls back to the slug when a title is missing", () => {
		expect(toArchitectureMeta("timeline-model", "/x", {}).title).toBe("timeline-model");
	});

	it("falls back to the description when there is no summary", () => {
		const result = toArchitectureMeta("a", "/x", { description: "The arithmetic" });

		expect(result.summary).toBe("The arithmetic");
	});

	it("sorts an unknown position last rather than first", () => {
		expect(toArchitectureMeta("a", "/x", {}).position).toBe(Number.MAX_SAFE_INTEGER);
	});

	it("falls back to production for an unrecognised status", () => {
		expect(toArchitectureMeta("a", "/x", { status: "wip" }).status).toBe("production");
	});

	it("falls back to platform for an unrecognised domain", () => {
		expect(toArchitectureMeta("a", "/x", { domain: "quantum" }).domain).toBe("platform");
	});

	it("drops empty strings out of a list", () => {
		const result = toArchitectureMeta("a", "/x", { inputs: ["Trim bounds", ""] });

		expect(result.inputs).toEqual(["Trim bounds"]);
	});

	it("treats a non-array list field as absent", () => {
		expect(toArchitectureMeta("a", "/x", { inputs: "Trim bounds" }).inputs).toEqual([]);
	});

	/// The rail is a summary of the page, not a second copy of its outline.
	it("keeps only level-two headings", () => {
		const result = toArchitectureMeta("a", "/x", {
			headings: [
				{ depth: 2, text: "Overview", id: "overview" },
				{ depth: 3, text: "A detail", id: "a-detail" },
			],
		});

		expect(result.headings.map((heading) => heading.id)).toEqual(["overview"]);
	});

	it("drops a heading with no anchor to link to", () => {
		const result = toArchitectureMeta("a", "/x", {
			headings: [{ depth: 2, text: "Overview", id: "" }],
		});

		expect(result.headings).toEqual([]);
	});
});

describe("sortDocs", () => {
	it("orders by position", () => {
		const sorted = sortDocs([meta({ slug: "b", position: 2 }), meta({ slug: "a", position: 1 })]);

		expect(sorted.map((doc) => doc.slug)).toEqual(["a", "b"]);
	});

	it("breaks a tie on title so the order never shifts between builds", () => {
		const sorted = sortDocs([
			meta({ slug: "b", title: "Zebra", position: 1 }),
			meta({ slug: "a", title: "Alpha", position: 1 }),
		]);

		expect(sorted.map((doc) => doc.slug)).toEqual(["a", "b"]);
	});

	it("leaves the input untouched", () => {
		const docs = [meta({ slug: "b", position: 2 }), meta({ slug: "a", position: 1 })];
		sortDocs(docs);

		expect(docs.map((doc) => doc.slug)).toEqual(["b", "a"]);
	});
});

describe("groupByDomain", () => {
	it("follows the Record → Polish → Share spine, not the reading order", () => {
		const sections = groupByDomain([
			meta({ slug: "export", domain: "pipeline", position: 6 }),
			meta({ slug: "record", domain: "capture", position: 1 }),
			meta({ slug: "overview", domain: "platform", position: 0 }),
		]);

		expect(sections.map((section) => section.domain)).toEqual(["platform", "capture", "pipeline"]);
	});

	it("drops a domain nothing is filed under", () => {
		const sections = groupByDomain([meta({ domain: "capture" })]);

		expect(sections).toHaveLength(1);
	});

	it("keeps reading order inside a section", () => {
		const sections = groupByDomain([
			meta({ slug: "state", domain: "editor", position: 8 }),
			meta({ slug: "seam", domain: "editor", position: 2 }),
		]);

		expect(sections[0].docs.map((doc) => doc.slug)).toEqual(["seam", "state"]);
	});
});

describe("neighbours", () => {
	const docs = [
		meta({ slug: "a", position: 0 }),
		meta({ slug: "b", position: 1 }),
		meta({ slug: "c", position: 2 }),
	];

	it("reads both directions from the middle", () => {
		const { previous, next } = neighbours(docs, "b");

		expect([previous?.slug, next?.slug]).toEqual(["a", "c"]);
	});

	it("has no previous at the start", () => {
		expect(neighbours(docs, "a").previous).toBeNull();
	});

	it("has no next at the end", () => {
		expect(neighbours(docs, "c").next).toBeNull();
	});

	it("reports neither for a slug that is not in the set", () => {
		expect(neighbours(docs, "missing")).toEqual({ previous: null, next: null });
	});

	/// The pager follows reading order, not whatever order the collection loaded in.
	it("ignores the order the docs arrived in", () => {
		const shuffled = [docs[2], docs[0], docs[1]];

		expect(neighbours(shuffled, "b").previous?.slug).toBe("a");
	});
});

describe("entrypointLabel", () => {
	it("keeps only the file name", () => {
		expect(entrypointLabel("apps/desktop/src-tauri/src/render/ops.rs")).toBe("ops.rs");
	});

	it("keeps the trailing slash so a directory does not read as a file", () => {
		expect(entrypointLabel("apps/desktop/src-tauri/src/capture/")).toBe("capture/");
	});

	it("passes a bare name through", () => {
		expect(entrypointLabel("lib.rs")).toBe("lib.rs");
	});
});
