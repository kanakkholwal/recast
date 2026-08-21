import { architecture } from "virtual:docvia/source";
import type { DocNodes } from "$lib/docs/render";
import { sortDocs, toArchitectureMeta } from "./meta.logic";
import type { ArchitectureMeta } from "./types";

/**
 * Architecture page data access. Server-only for the same reason as the blog:
 * importing the collection pulls in every compiled markdown module, so a
 * universal `load` would put all twelve documents in the client bundle.
 */

export interface ArchitecturePage {
	meta: ArchitectureMeta;
	/** docvia's compiled node tree — plain JSON, so it serializes to the client. */
	content: DocNodes;
}

/**
 * Every page, in reading order.
 *
 * Resolves each page rather than reading `getPages()` data directly: that cache
 * is populated asynchronously, so its first synchronous call hands back entries
 * whose `data` is still an empty object.
 */
export async function listDocs(): Promise<ArchitectureMeta[]> {
	const entries = architecture.getPages();
	const docs = await Promise.all(
		entries.map(async (entry) => {
			const page = await architecture.getPage(entry.slugs);
			if (!page) return null;
			return toArchitectureMeta(
				entry.slugs.join("/"),
				entry.url,
				page.data as Record<string, unknown>,
			);
		}),
	);

	return sortDocs(docs.filter((doc): doc is ArchitectureMeta => doc !== null));
}

/** One page by slug, or `null` when it does not exist. */
export async function getDoc(slug: string): Promise<ArchitecturePage | null> {
	const page = await architecture.getPage([slug]);
	if (!page) return null;

	return {
		meta: toArchitectureMeta(slug, page.url, page.data as Record<string, unknown>),
		content: page.content as DocNodes,
	};
}
