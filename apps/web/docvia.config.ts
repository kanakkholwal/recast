import { defineConfig } from "@docvia/cli";
import { shiki } from "@docvia/plugin-shiki";
import { createSvelteRenderer } from "@docvia/renderer-svelte/node";
import { z } from "zod";

type Renderer = ReturnType<typeof createSvelteRenderer>;

/**
 * Fields beyond docvia's built-ins (title, description, tags, slug, order).
 * Validated at compile time, so a post that forgets its byline or date fails the
 * build instead of shipping an article with a blank dateline.
 */
const frontmatter = z.object({
	author: z.string(),
	// YAML turns an unquoted `2026-07-13` into a Date already; coercing means a
	// quoted string behaves identically rather than silently differing.
	date: z.coerce.date(),
	// Drafts are excluded from the production listing and 404 on direct hit.
	// Flip to `true` to publish.
	published: z.boolean().default(false),
});

/**
 * docvia's `toPageMeta()` copies ONLY its built-in fields onto the `meta` that
 * each compiled page exports. Custom frontmatter is validated by the schema
 * above and then dropped, so `collection.getPage().data` would carry no `date`,
 * `author`, or `published` at runtime — the schema would typecheck a blog that
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
	sourceDir: "content/blog",
	outDir: ".docvia",

	// Named explicitly so the virtual module exports `blog` (the implicit default
	// would be `docs` at baseUrl `/`) and page URLs resolve to `/blog/<slug>`.
	collections: [{ name: "blog", sourceDir: "content/blog", baseUrl: "/blog" }],

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
	],
});
