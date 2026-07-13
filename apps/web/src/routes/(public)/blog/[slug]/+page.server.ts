import { error } from "@sveltejs/kit";
import { getPost, listPosts } from "$lib/blog";
import type { EntryGenerator, PageServerLoad } from "./$types";

export const prerender = true;

// The prerenderer only finds pages it can reach by crawling links. Enumerating
// the slugs explicitly keeps the build independent of what the index happens to
// link to, so a post is still prerendered if the listing ever filters it out.
export const entries: EntryGenerator = async () => {
	const posts = await listPosts();
	return posts.map((post) => ({ slug: post.slug }));
};

export const load: PageServerLoad = async ({ params }) => {
	const post = await getPost(params.slug);
	// Also the unpublished-draft path in production: `getPost` returns null, and
	// a draft URL should not be a live page.
	if (!post) error(404, "Article not found");
	return post;
};
