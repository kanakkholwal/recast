import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { danglingEdges, mappedSlugs, SYSTEM_EDGES, SYSTEM_NODES } from "./system-map";

const CONTENT_DIR = fileURLToPath(new URL("../../../content/architecture", import.meta.url));

describe("system map", () => {
	it("draws no edge to a node that does not exist", () => {
		expect(danglingEdges()).toEqual([]);
	});

	it("declares every node once", () => {
		const ids = SYSTEM_NODES.map((node) => node.id);

		expect(ids).toHaveLength(new Set(ids).size);
	});

	it("draws each connection once", () => {
		const pairs = SYSTEM_EDGES.map((edge) => `${edge.source}->${edge.target}`);

		expect(pairs).toHaveLength(new Set(pairs).size);
	});

	/// A node nothing connects to is a box the reader cannot place.
	it("connects every node to something", () => {
		const connected = new Set(SYSTEM_EDGES.flatMap((edge) => [edge.source, edge.target]));
		const orphans = SYSTEM_NODES.filter((node) => !connected.has(node.id)).map((node) => node.id);

		expect(orphans).toEqual([]);
	});

	/// The map doubles as the navigation, so a bad slug is a dead click.
	it("links only to pages that exist", () => {
		const missing = mappedSlugs().filter((slug) => !existsSync(`${CONTENT_DIR}/${slug}.md`));

		expect(missing).toEqual([]);
	});

	it("covers every architecture page that has a place in the flow", () => {
		const pages = readdirSync(CONTENT_DIR)
			.filter((file) => file.endsWith(".md"))
			.map((file) => file.replace(/\.md$/, ""));
		const linked = new Set(mappedSlugs());
		// The overview and the two boundary pages describe the whole map rather
		// than sitting at one point in it.
		const notInFlow = new Set(["system-overview", "editor-host-seam", "media-decode-workers"]);
		const uncovered = pages.filter((slug) => !linked.has(slug) && !notInFlow.has(slug));

		expect(uncovered).toEqual([]);
	});
});
