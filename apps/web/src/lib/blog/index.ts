import { blog } from "virtual:docvia/source";
import { dev } from "$app/environment";
import { type DocHeading, docHeadings } from "$lib/docs/headings";
import type { DocNodes } from "$lib/docs/render";

/**
 * Blog data access. Server-only on purpose: importing the docvia collection
 * pulls in every compiled markdown module, so keeping it out of universal
 * `load` functions is what stops all four articles from landing in the client
 * bundle. Routes read it from `+page.server.ts`; only the small objects below
 * cross to the browser.
 */

/** One article's metadata, as the listing and the article header need it. */
export interface PostMeta {
	slug: string;
	url: string;
	title: string;
	description: string;
	author: string;
	/** ISO 8601. A string, not a Date, so it survives SvelteKit serialization. */
	date: string;
	tags: string[];
	published: boolean;
	readingMinutes: number;
	/** Derived by docvia from the markdown, for the contents rail. */
	headings: DocHeading[];
}

/** A full article: its metadata plus the compiled render tree. */
export interface Post {
	meta: PostMeta;
	/** docvia's compiled node tree — plain JSON, so it serializes to the client. */
	content: DocNodes;
}

/**
 * Drafts (`published: false`) are visible while developing and hidden in
 * production, so an unfinished post can be previewed locally without leaking to
 * the live site or the sitemap.
 */
function isVisible(published: boolean): boolean {
	return published || dev;
}

/**
 * docvia serializes `meta` with `JSON.stringify`, so the `Date` that Zod coerced
 * at build time arrives here as an ISO string even though the generated type
 * still says `Date`. Normalize both shapes rather than trusting either.
 */
function toIsoDate(value: unknown): string {
	const date = value instanceof Date ? value : new Date(String(value ?? ""));
	return Number.isNaN(date.getTime()) ? new Date(0).toISOString() : date.toISOString();
}

/** Words in the compiled tree, for a reading estimate. */
function countWords(node: unknown): number {
	if (!node) return 0;
	if (Array.isArray(node)) return node.reduce<number>((sum, child) => sum + countWords(child), 0);
	if (typeof node !== "object") return 0;

	const candidate = node as { kind?: string; value?: string; children?: unknown };
	// Raw `html` nodes are pre-highlighted code, which nobody reads at prose speed.
	if (candidate.kind === "text") {
		return candidate.value?.trim() ? candidate.value.trim().split(/\s+/).length : 0;
	}
	return countWords(candidate.children);
}

/** 220 wpm is the usual prose reading estimate; never report less than a minute. */
function readingMinutes(content: unknown): number {
	return Math.max(1, Math.round(countWords(content) / 220));
}

function toMeta(
	slug: string,
	url: string,
	data: Record<string, unknown>,
	content: unknown,
): PostMeta {
	return {
		slug,
		url,
		title: String(data.title ?? slug),
		description: String(data.description ?? ""),
		author: String(data.author ?? ""),
		date: toIsoDate(data.date),
		tags: Array.isArray(data.tags) ? data.tags.map(String) : [],
		published: data.published === true,
		readingMinutes: readingMinutes(content),
		headings: docHeadings(data.headings),
	};
}

/**
 * Every visible article, newest first.
 *
 * Note it resolves each page rather than reading `blog.getPages()` directly:
 * `getPages()` reads an eager-module cache that is populated asynchronously, so
 * its FIRST synchronous call returns pages whose `data` is an empty object. Its
 * `slugs` are reliable (they come from the route keys), so we take those and
 * fetch each page for the real frontmatter.
 */
export async function listPosts(): Promise<PostMeta[]> {
	const entries = blog.getPages();
	const posts = await Promise.all(
		entries.map(async (entry) => {
			const page = await blog.getPage(entry.slugs);
			if (!page) return null;
			return toMeta(
				entry.slugs.join("/"),
				entry.url,
				page.data as Record<string, unknown>,
				page.content,
			);
		}),
	);

	return posts
		.filter((post): post is PostMeta => post !== null && isVisible(post.published))
		.sort((a, b) => b.date.localeCompare(a.date));
}

/** One article by slug, or `null` when it does not exist or is an unpublished draft. */
export async function getPost(slug: string): Promise<Post | null> {
	const page = await blog.getPage([slug]);
	if (!page) return null;

	const data = page.data as Record<string, unknown>;
	const meta = toMeta(slug, page.url, data, page.content);
	if (!isVisible(meta.published)) return null;

	return { meta, content: page.content as DocNodes };
}
