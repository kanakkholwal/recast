/**
 * Pulling mermaid source back out of docvia's compiled node tree.
 *
 * The `recast/mermaid-blocks` plugin in `docvia.config.ts` tags these at build
 * time, because the language is known in the IR but the renderer drops it. By
 * the time the tree gets here a mermaid fence looks like any other code block
 * apart from that class.
 *
 * Both node shapes are handled: docvia currently wraps the plugin's markup in
 * an `html` node, and an `element` tree of `pre > code` is what it would emit
 * if that ever changed. Guessing one would mean a diagram silently rendering as
 * source text.
 */

import type { DocNode } from "./render";

/** Must match `MERMAID_CLASS` in `docvia.config.ts`. */
const MERMAID_CLASS = /\bdocvia-mermaid\b/;

/** Concatenate every text node under `node`, ignoring element structure. */
function textOf(node: DocNode): string {
	if (node.kind === "text") return node.value;
	if (node.kind === "element" || node.kind === "fragment" || node.kind === "component") {
		return (node.children ?? []).map(textOf).join("");
	}
	return "";
}

function classOf(node: DocNode): string {
	if (node.kind !== "element") return "";
	const props = node.props ?? {};
	const value = props.class ?? props.className;
	return typeof value === "string" ? value : "";
}

/** Undo the entity escaping a code block picked up on the way through HTML. */
function unescapeHtml(value: string): string {
	return (
		value
			.replace(/&lt;/g, "<")
			.replace(/&gt;/g, ">")
			.replace(/&quot;/g, '"')
			.replace(/&#39;/g, "'")
			// Ampersand last, so `&amp;lt;` doesn't decode twice into `<`.
			.replace(/&amp;/g, "&")
	);
}

/**
 * The diagram source in `node`, or null when it isn't a mermaid block.
 *
 * Matches on the build-time class rather than sniffing the source for
 * `flowchart`/`graph`, so a prose code sample that happens to contain mermaid
 * syntax is never hijacked into a diagram.
 */
export function mermaidSourceOf(node: DocNode): string | null {
	if (node.kind === "element") {
		if (node.tag !== "pre" && node.tag !== "code") return null;
		const marked =
			MERMAID_CLASS.test(classOf(node)) ||
			(node.children ?? []).some((child) => MERMAID_CLASS.test(classOf(child)));
		if (!marked) return null;
		const source = textOf(node).trim();
		return source.length > 0 ? source : null;
	}

	if (node.kind === "html") {
		if (!MERMAID_CLASS.test(node.value)) return null;
		// Non-greedy so several blocks on one page can't merge into one match.
		const match = node.value.match(/<code[^>]*>([\s\S]*?)<\/code>/i);
		if (!match) return null;
		const source = unescapeHtml(match[1].replace(/<[^>]+>/g, "")).trim();
		return source.length > 0 ? source : null;
	}

	return null;
}
