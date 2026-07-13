import { listPosts } from "$lib/blog";
import { getPublicEnv } from "$lib/env/public";
import { TOOLS } from "$lib/tools/registry";
import type { RequestHandler } from "./$types";

export const prerender = true;

// Public, indexable pages. Private areas (dashboard, admin, auth, share) are
// intentionally excluded — they're noindex and disallowed in robots.txt.
const STATIC_PATHS = [
	"/",
	"/features",
	"/extensions",
	"/pricing",
	"/download",
	"/changelog",
	"/blog",
	"/privacy-policy",
	"/terms-of-service",
	"/tools",
	// The screenshot editor is not a WebCodecs worker op, so it is absent from
	// TOOLS and has to be listed by hand. Its landing page is the indexable one;
	// /tools/screenshot-editor/edit is the client-only app and stays out.
	"/tools/screenshot-editor",
];

function siteOrigin(fallback: string): string {
	try {
		return getPublicEnv().PUBLIC_APP_URL.replace(/\/+$/, "");
	} catch {
		return fallback.replace(/\/+$/, "");
	}
}

export const GET: RequestHandler = async ({ url }) => {
	const origin = siteOrigin(url.origin);
	// Only published posts: `listPosts` drops drafts in production, so an
	// unfinished article is never advertised to a crawler.
	const posts = await listPosts();
	const paths = [
		...STATIC_PATHS,
		...TOOLS.map((t) => `/tools/${t.slug}`),
		...posts.map((post) => post.url),
	];
	const urls = paths
		.map((p) => `  <url>\n    <loc>${origin}${p === "/" ? "" : p}</loc>\n  </url>`)
		.join("\n");
	const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`;
	return new Response(body, {
		headers: {
			"content-type": "application/xml",
			"cache-control": "max-age=0, s-maxage=3600",
		},
	});
};
