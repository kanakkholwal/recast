import { describe, expect, it } from "vitest";
import { buildToolJsonLd } from "$lib/components/ToolPage.logic";
import { TOOLS } from "./registry";
import { buildEditorJsonLd, EDITOR_FAQ } from "./screenshot-editor";

const ORIGIN = "https://recast.li";

type Node = Record<string, unknown>;
const parse = (s: string) => JSON.parse(s) as Node[];
const byType = (nodes: Node[], type: string) => nodes.find((n) => n["@type"] === type);

describe("tool registry FAQs", () => {
	it("gives every tool enough answers to be worth a page", () => {
		for (const tool of TOOLS) {
			expect(tool.faq.length, `${tool.slug} has too few FAQs`).toBeGreaterThanOrEqual(6);
		}
	});

	it("never repeats a question within one tool", () => {
		for (const tool of TOOLS) {
			const qs = tool.faq.map((f) => f.q.toLowerCase());
			expect(new Set(qs).size, `${tool.slug} repeats a question`).toBe(qs.length);
		}
	});

	it("leads with the tool-specific answer, not the boilerplate", () => {
		for (const tool of TOOLS) {
			expect(tool.faq[0]?.q, `${tool.slug} starts with a generic question`).toMatch(/uploaded/i);
			// The shared block is appended, so the last question is a common one.
			expect(tool.faq.at(-1)?.q).toMatch(/fan spinning/i);
		}
	});

	it("answers something, not nothing", () => {
		for (const tool of TOOLS) {
			for (const f of tool.faq) {
				expect(f.a.length, `${tool.slug}: "${f.q}" has a stub answer`).toBeGreaterThan(40);
			}
		}
	});
});

describe("buildToolJsonLd", () => {
	const nodes = parse(buildToolJsonLd(TOOLS[0]!, ORIGIN));

	it("emits the app, the breadcrumb trail and the FAQ", () => {
		expect(nodes.map((n) => n["@type"])).toEqual([
			"SoftwareApplication",
			"BreadcrumbList",
			"FAQPage",
		]);
	});

	it("uses absolute URLs so the nodes resolve", () => {
		const app = byType(nodes, "SoftwareApplication")!;
		expect(app.url).toBe(`${ORIGIN}/tools/${TOOLS[0]!.slug}`);
		expect(app["@id"]).toContain(ORIGIN);
	});

	it("walks Home -> Tools -> the tool", () => {
		const crumbs = byType(nodes, "BreadcrumbList")!.itemListElement as Node[];
		expect(crumbs.map((c) => c.position)).toEqual([1, 2, 3]);
		expect(crumbs[1]!.item).toBe(`${ORIGIN}/tools`);
		expect(crumbs[2]!.name).toBe(TOOLS[0]!.title);
	});

	it("carries every on-page question, so the two can't drift", () => {
		const faq = byType(nodes, "FAQPage")!.mainEntity as Node[];
		expect(faq).toHaveLength(TOOLS[0]!.faq.length);
	});

	it("states the price as free rather than omitting it", () => {
		const app = byType(nodes, "SoftwareApplication")!;
		expect(app.isAccessibleForFree).toBe(true);
		expect(app.offers).toMatchObject({ price: "0", priceCurrency: "USD" });
	});

	it("stays valid JSON for every tool", () => {
		for (const tool of TOOLS) {
			expect(() => parse(buildToolJsonLd(tool, ORIGIN))).not.toThrow();
		}
	});
});

describe("buildEditorJsonLd", () => {
	const nodes = parse(buildEditorJsonLd(ORIGIN));

	it("emits the app, the breadcrumb trail and the FAQ", () => {
		expect(nodes.map((n) => n["@type"])).toEqual([
			"SoftwareApplication",
			"BreadcrumbList",
			"FAQPage",
		]);
	});

	it("lists what the editor actually does", () => {
		const app = byType(nodes, "SoftwareApplication")!;
		expect((app.featureList as string[]).length).toBeGreaterThanOrEqual(6);
		expect(app.url).toBe(`${ORIGIN}/tools/screenshot-editor`);
	});

	it("matches the on-page FAQ one for one", () => {
		const faq = byType(nodes, "FAQPage")!.mainEntity as Node[];
		expect(faq).toHaveLength(EDITOR_FAQ.length);
		expect(EDITOR_FAQ.length).toBeGreaterThanOrEqual(10);
	});
});
