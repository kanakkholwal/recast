import { listDocs } from "$lib/architecture";
import type { PageServerLoad } from "./$types";

// Static content: docvia compiles the markdown at build time, so these pages cost nothing per request.
export const prerender = true;

export const load: PageServerLoad = async () => ({
	docs: await listDocs(),
});
