<script lang="ts">
	import { page } from "$app/state";
	import { buildOgUrl } from "./SeoMeta.logic";

	type Props = {
		title: string;
		description: string;
		/** Eyebrow shown above the title in the generated OG image (e.g. "Pricing"). */
		eyebrow?: string;
		/** Full title for the browser tab. Defaults to `${title} - Recast`. */
		pageTitle?: string;
		/** Override the rendered OG image URL (absolute). Skips the takumi generator. */
		ogImage?: string;
		/** Canonical path override. Defaults to the current pathname. */
		canonicalPath?: string;
		/** "website" by default — switch to "article" for blog/changelog posts. */
		ogType?: "website" | "article";
	};

	let {
		title,
		description,
		eyebrow,
		pageTitle,
		ogImage,
		canonicalPath,
		ogType = "website",
	}: Props = $props();

	const origin = $derived(page.url.origin);
	const canonical = $derived(`${origin}${canonicalPath ?? page.url.pathname}`);

	const generatedOg = $derived(buildOgUrl(origin, title, description, eyebrow));

	const ogUrl = $derived(ogImage ?? generatedOg);
	const headTitle = $derived(pageTitle ?? `${title} - Recast`);
</script>

<svelte:head>
	<title>{headTitle}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={canonical} />

	<meta property="og:type" content={ogType} />
	<meta property="og:site_name" content="Recast" />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={description} />
	<meta property="og:url" content={canonical} />
	<meta property="og:image" content={ogUrl} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:image:alt" content={title} />

	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={description} />
	<meta name="twitter:image" content={ogUrl} />
</svelte:head>
