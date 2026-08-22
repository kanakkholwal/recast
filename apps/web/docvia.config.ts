import { defineConfig } from "@docvia/cli";
import { shiki } from "@docvia/plugin-shiki";
import { createSvelteRenderer } from "@docvia/renderer-svelte/node";
import { z } from "zod";
import { sourceUrl } from "./src/lib/docs/source-links.ts";

type Renderer = ReturnType<typeof createSvelteRenderer>;

/** Class the browser looks for to swap a fenced block for a rendered diagram. */
const MERMAID_CLASS = "docvia-mermaid";

type DocviaPlugin = NonNullable<Parameters<typeof defineConfig>[0]["plugins"]>[number];
type IRDoc = Parameters<NonNullable<DocviaPlugin["beforeRender"]>>[0];
type IRNode = IRDoc["children"][number];

function escapeHtml(value: string): string {
	return value
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;");
}

function mermaidBlocks(): DocviaPlugin {
	const mark = (nodes: readonly IRNode[]): IRNode[] =>
		nodes.map((node) => {
			if (node.type === "code-block" && String(node.props.lang ?? "").trim() === "mermaid") {
				const source = String(node.props.value ?? "");
				return {
					...node,
					props: {
						...node.props,
						html: `<pre class="${MERMAID_CLASS}"><code>${escapeHtml(source)}</code></pre>`,
					},
				};
			}
			if (node.children.length > 0) return { ...node, children: mark(node.children) };
			return node;
		});

	return {
		name: "recast/mermaid-blocks",
		version: "1.0.0",
		phase: "post",
		priority: 200,
		cacheKey: () => `recast-mermaid@1|${MERMAID_CLASS}`,
		beforeRender(doc) {
			return { ...doc, children: mark(doc.children) };
		},
	};
}

const post = z.object({
	kind: z.literal("post"),
	author: z.string(),
	// YAML turns an unquoted `2026-07-13` into a Date already; coercing means a
	// quoted string behaves identically rather than silently differing.
	date: z.coerce.date(),
	// Drafts are excluded from the production listing and 404 on direct hit.
	// Flip to `true` to publish.
	published: z.boolean().default(false),
});

/**
 * An architecture page's structured facts. These render as the at-a-glance panel
 * above the prose and are what an agent reads instead of the whole document, so
 * they are required rather than optional: a subsystem page that cannot state its
 * inputs, outputs, and invariants fails the build.
 */
const architecture = z.object({
	kind: z.literal("architecture"),
	position: z.number().int().min(0),
	status: z.enum(["production", "beta", "planned"]),
	domain: z.enum(["capture", "editor", "render", "pipeline", "platform", "cloud", "agent"]),
	summary: z.string().min(1),
	inputs: z.array(z.string()).min(1),
	outputs: z.array(z.string()).min(1),
	entrypoints: z.array(z.string()).min(1),
	invariants: z.array(z.string()).min(1),
});

/**
 * Turn a file reference into a link to the source.
 *
 * Architecture pages cite real paths constantly. As inline code they are inert
 * and read as noise; as links they are the fastest way into the code. Only
 * unambiguous paths are linked (see `repoPath`), so a bare `mod.rs` stays code.
 */
function sourceLinks(): DocviaPlugin {
	const link = (nodes: readonly IRNode[]): IRNode[] =>
		nodes.map((node) => {
			if (node.type === "inline-code") {
				const href = sourceUrl(String(node.props.value ?? ""));
				if (!href) return node;
				return {
					type: "link",
					props: { href, title: null, class: "source-link" },
					children: [node],
				} as IRNode;
			}
			if (node.children.length > 0) return { ...node, children: link(node.children) };
			return node;
		});

	return {
		name: "recast/source-links",
		version: "1.0.0",
		phase: "post",
		priority: 210,
		cacheKey: () => "recast-source-links@1",
		beforeRender(doc) {
			return { ...doc, children: link(doc.children) };
		},
	};
}

/**
 * Fields beyond docvia's built-ins (title, description, tags, slug, order).
 * Validated at compile time, so a post that forgets its byline or an
 * architecture page that forgets its invariants fails the build.
 *
 * One schema covers both collections because docvia applies a single frontmatter
 * schema to every file it compiles, and it never tells the schema which
 * collection a file came from. `kind` is that discriminator, written by hand in
 * each file: a union without one would report a mistyped architecture field as
 * "missing author".
 */
const frontmatter = z.discriminatedUnion("kind", [post, architecture]);

/**
 * docvia's `toPageMeta()` copies ONLY its built-in fields onto the `meta` that
 * each compiled page exports. Custom frontmatter is validated by the schema
 * above and then dropped, so `collection.getPage().data` would carry no `date`,
 * `author`, or `published` at runtime, so the schema would typecheck a blog that
 * cannot render a byline.
 *
 * Until that is fixed upstream, wrap the Svelte adapter and re-emit `meta` with
 * the validated frontmatter merged in UNDERNEATH the built-ins, so title,
 * headings, and slug stay canonical and only the extra fields are added.
 */
function preserveCustomFrontmatter(base: Renderer): Renderer {
	const PREFIX = "export const meta = ";
	return {
		...base,
		async renderPage(doc) {
			const rendered = await base.renderPage(doc);
			// The adapter emits `meta` first, as a pretty-printed JSON literal, so
			// its object ends at the first `\n};`.
			const end = rendered.code.indexOf("\n};");
			if (!rendered.code.startsWith(PREFIX) || end < 0) {
				// Fail the build rather than quietly losing every byline and date.
				throw new Error(
					"[docvia.config] could not patch the generated `meta` export: the " +
						"renderer's codegen shape changed. Update preserveCustomFrontmatter().",
				);
			}
			const builtIn = JSON.parse(rendered.code.slice(PREFIX.length, end + 2));
			const meta = { ...doc.frontmatter, ...builtIn };
			return {
				...rendered,
				code: `${PREFIX}${JSON.stringify(meta, null, 2)};${rendered.code.slice(end + 3)}`,
			};
		},
	};
}

export default defineConfig({
	// The parent of every collection, not a collection itself: the Vite plugin
	// watches this one path for hot reload, so pointing it at `content/blog`
	// would leave architecture edits needing a dev-server restart.
	sourceDir: "content",
	outDir: ".docvia",

	// Named explicitly so the virtual module exports `blog`/`architecture` (the
	// implicit default would be one `docs` collection at baseUrl `/`) and page
	// URLs resolve under the matching route.
	collections: [
		{ name: "blog", sourceDir: "content/blog", baseUrl: "/blog" },
		{ name: "architecture", sourceDir: "content/architecture", baseUrl: "/architecture" },
	],

	frontmatter,

	renderer: preserveCustomFrontmatter(createSvelteRenderer()),

	// Syntax highlighting is a build-time plugin: the highlighted HTML is baked
	// into the IR, so no highlighter ships to the browser. The current posts are
	// pure prose, but this is ready for the ones that aren't.
	plugins: [
		shiki({
			theme: "github-dark",
			langs: [
				"javascript",
				"typescript",
				"svelte",
				"rust",
				"html",
				"css",
				"bash",
				"json",
				"toml",
				"yaml",
			],
		}),
		// Must stay AFTER shiki: it rewrites shiki's unknown-language fallback.
		mermaidBlocks(),
		sourceLinks(),
	],
});
