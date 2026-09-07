import { error } from "@sveltejs/kit";
import { TOOLS, toolBySlug } from "$lib/tools/registry";
import type { EntryGenerator, PageLoad } from "./$types";

// Prerendered to static HTML for SEO; the conversion JS hydrates and runs client-side.
export const prerender = true;

export const entries: EntryGenerator = () => TOOLS.map((t) => ({ slug: t.slug }));

export const load: PageLoad = ({ params }) => {
	const tool = toolBySlug(params.slug);
	if (!tool) throw error(404, "Tool not found");
	return { tool };
};
