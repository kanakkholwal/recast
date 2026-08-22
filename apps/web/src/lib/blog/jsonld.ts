import { buildOgUrl } from "$lib/components/SeoMeta.logic";
import type { PostMeta } from "./index";

/**
 * BlogPosting + BreadcrumbList structured data for one article. Emitted in the
 * post's `<svelte:head>`, on top of the site-wide Organization/WebSite graph the
 * root layout adds. Takes `origin` as a primitive so it stays free of reactive
 * state and is trivially testable.
 */
export function buildPostJsonLd(origin: string, meta: PostMeta): string {
	const url = `${origin}${meta.url}`;
	const image = buildOgUrl(origin, meta.title, meta.description, "Blog");
	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "BlogPosting",
			headline: meta.title,
			description: meta.description,
			url,
			mainEntityOfPage: { "@type": "WebPage", "@id": url },
			datePublished: meta.date,
			dateModified: meta.date,
			author: { "@type": "Person", name: meta.author },
			publisher: {
				"@type": "Organization",
				name: "Recast",
				url: origin,
				logo: { "@type": "ImageObject", url: `${origin}/logo.png` },
			},
			image: [image],
			keywords: meta.tags.join(", "),
			articleSection: meta.tags[0] ?? "Blog",
		},
		{
			"@context": "https://schema.org",
			"@type": "BreadcrumbList",
			itemListElement: [
				{ "@type": "ListItem", position: 1, name: "Home", item: origin },
				{ "@type": "ListItem", position: 2, name: "Blog", item: `${origin}/blog` },
				{ "@type": "ListItem", position: 3, name: meta.title, item: url },
			],
		},
	]);
}
