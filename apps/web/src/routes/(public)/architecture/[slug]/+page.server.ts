import { error } from "@sveltejs/kit";
import { getDoc, listDocs } from "$lib/architecture";
import { neighbours } from "$lib/architecture/meta.logic";
import type { EntryGenerator, PageServerLoad } from "./$types";

export const prerender = true;

// The prerenderer only finds pages it can reach by crawling links, and the
// system map's links are built in the browser. Enumerating the slugs keeps every
// page prerendered whether or not the index happens to link to it.
export const entries: EntryGenerator = async () => {
	const docs = await listDocs();
	return docs.map((doc) => ({ slug: doc.slug }));
};

export const load: PageServerLoad = async ({ params }) => {
	const doc = await getDoc(params.slug);
	if (!doc) error(404, "Architecture page not found");

	return { ...doc, ...neighbours(await listDocs(), params.slug) };
};
