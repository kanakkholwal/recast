import { listPosts } from "$lib/blog";
import type { PageServerLoad } from "./$types";

// Static content: rendered once at build time, served as HTML. The markdown is
// compiled by docvia during the build, so nothing about it costs anything at
// request time.
export const prerender = true;

export const load: PageServerLoad = async () => ({
	posts: await listPosts(),
});
