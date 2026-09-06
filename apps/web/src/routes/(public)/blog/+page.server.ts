import { listPosts } from "$lib/blog";
import type { PageServerLoad } from "./$types";

// Static content: docvia compiles the markdown during the build, so nothing costs anything per request.
export const prerender = true;

export const load: PageServerLoad = async () => ({
	posts: await listPosts(),
});
