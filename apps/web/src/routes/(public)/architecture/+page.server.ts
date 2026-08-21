import { listDocs } from "$lib/architecture";
import type { PageServerLoad } from "./$types";

// Static content: docvia compiles the markdown at build time, so nothing about
// these pages costs anything at request time.
export const prerender = true;

export const load: PageServerLoad = async () => ({
	docs: await listDocs(),
});
