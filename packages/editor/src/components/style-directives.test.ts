import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { parse } from "svelte/compiler";
import { describe, expect, it } from "vitest";

const ROOT = join(import.meta.dirname, "..");

function svelteFiles(dir: string): string[] {
	const out: string[] = [];
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) out.push(...svelteFiles(path));
		else if (entry.name.endsWith(".svelte")) out.push(path);
	}
	return out;
}

interface Node {
	type?: string;
	name?: string;
	attributes?: { type: string; name: string }[];
	[key: string]: unknown;
}

function clashOn(node: Node): string | null {
	const attrs = node.attributes;
	if (!Array.isArray(attrs)) return null;
	if (!attrs.some((a) => a.type === "Attribute" && a.name === "style")) return null;
	const directives = attrs.filter((a) => a.type === "StyleDirective").map((a) => a.name);
	if (directives.length === 0) return null;
	return `<${node.name}> style={...} + style:${directives.join(" style:")}`;
}

function collect(node: Node | null, found: string[]) {
	if (!node || typeof node !== "object") return;
	const clash = clashOn(node);
	if (clash) found.push(clash);
	for (const value of Object.values(node)) {
		if (Array.isArray(value)) for (const child of value) collect(child as Node, found);
		else if (value && typeof value === "object") collect(value as Node, found);
	}
}

function offenders(source: string, filename: string): string[] {
	const found: string[] = [];
	collect(parse(source, { modern: true, filename }).fragment as unknown as Node, found);
	return found;
}

/**
 * A `style:` directive reserves its property name and deletes it from the
 * `style={...}` string on the same element, whatever the directive resolves to.
 * That silently dropped every text annotation's colour, so the glyphs took the
 * page's inherited colour and vanished against the video.
 */
describe("inline style ownership", () => {
	it("gives no element both a style attribute and a style directive", () => {
		const clashes = svelteFiles(ROOT).flatMap((file) => {
			const relative = file.slice(ROOT.length + 1).replaceAll("\\", "/");
			return offenders(readFileSync(file, "utf8"), relative).map((at) => `${relative} ${at}`);
		});

		expect(clashes).toEqual([]);
	});

	it("catches the shape it exists to catch", () => {
		const bad = `<div style={s} style:color={c}>x</div>`;

		expect(offenders(bad, "bad.svelte")).toEqual(["<div> style={...} + style:color"]);
	});
});
