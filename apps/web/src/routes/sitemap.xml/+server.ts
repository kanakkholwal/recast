import { listDocs } from "$lib/architecture";
import { listPosts } from "$lib/blog";
import { getPublicEnv } from "$lib/env/public";
import { TOOLS } from "$lib/tools/registry";
import type { RequestHandler } from "./$types";

export const prerender = true;

// Public, indexable pages; private areas are noindex and disallowed in robots.txt.
const STATIC_PATHS = [
	"/",
	"/features",
	"/extensions",
	"/pricing",
	"/download",
	"/changelog",
	"/blog",
	"/architecture",
	"/privacy-policy",
	"/terms-of-service",
	"/tools",
	// The screenshot editor isn't a WebCodecs tool, so it is listed by hand; only its landing page is indexable.
	"/tools/screenshot-editor",
	// One route: the editor is a client-only island on this same URL.
	"/playground",
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
	// `listPosts` drops drafts in production, so an unfinished article is never advertised to a crawler.
	const posts = await listPosts();
	const architecture = await listDocs();
	// `lastmod` only where we know it: a fabricated date on every static page trains crawlers to ignore the signal.
	const entries: Array<{ path: string; lastmod?: string }> = [
		...STATIC_PATHS.map((path) => ({ path })),
		...TOOLS.map((t) => ({ path: `/tools/${t.slug}` })),
		...posts.map((post) => ({ path: post.url, lastmod: post.date.slice(0, 10) })),
		...architecture.map((doc) => ({ path: `/architecture/${doc.slug}` })),
	];
	const urls = entries
		.map(({ path, lastmod }) => {
			const loc = `    <loc>${origin}${path === "/" ? "" : path}</loc>`;
			const mod = lastmod ? `\n    <lastmod>${lastmod}</lastmod>` : "";
			return `  <url>\n${loc}${mod}\n  </url>`;
		})
		.join("\n");
	const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`;
	return new Response(body, {
		headers: {
			"content-type": "application/xml",
			"cache-control": "max-age=0, s-maxage=3600",
		},
	});
};
