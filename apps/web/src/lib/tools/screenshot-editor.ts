/**
 * Content + structured data for the screenshot editor's landing page.
 *
 * The editor is not a WebCodecs worker op, so it sits outside the `TOOLS`
 * registry and carries its own copy. Kept framework-free so the JSON-LD can be
 * built at prerender time and asserted in tests.
 */

export interface EditorFaq {
	q: string;
	a: string;
}

/** The project our editor is a Svelte port of. Credited in the editor's top bar
 * and in packages/application/NOTICE.md (Apache-2.0 attribution). */
export const UPSTREAM_URL = "https://github.com/KartikLabhshetwar/screenshot-studio";

export const EDITOR_TITLE = "Free Screenshot Editor";

export const EDITOR_DESCRIPTION =
	"Turn a plain screenshot into something worth shipping. Add a gradient backdrop, rounded corners, a shadow, a browser mockup, and a 3D tilt, then export at up to 4x. Runs entirely in your browser.";

/** Answers double as the on-page FAQ and the FAQPage schema, so they stay in sync. */
export const EDITOR_FAQ: EditorFaq[] = [
	{
		q: "Is my screenshot uploaded to a server?",
		a: "No. The editor runs entirely in your browser and the image never leaves your device. There is no upload step and no account.",
	},
	{
		q: "Is it really free?",
		a: "Yes. Every feature is free, there is no watermark, and there is no sign-up. The desktop app is where we sell a paid plan, not this tool.",
	},
	{
		q: "What can I export?",
		a: "PNG or JPG at up to 4x for crisp output on retina displays, or copy straight to your clipboard. If you add a motion preset you can also export an MP4 clip.",
	},
	{
		q: "Can I put my screenshot in a browser window?",
		a: "Yes. There are Safari and Chrome window frames in light and dark, plus phone and tablet device frames. You can set the address bar text to your own URL.",
	},
	{
		q: "What image formats can I open?",
		a: "PNG, JPG, and WebP. You can upload a file, paste from the clipboard, or drag an image straight onto the page.",
	},
	{
		q: "Does it work offline?",
		a: "Once the page has loaded, yes. The editing and export both happen on your device, so it keeps working without a connection.",
	},
	{
		q: "What size should I export for Twitter, LinkedIn or a blog?",
		a: "Export at 2x and let the platform downscale: it always looks better than uploading something small. A 16:9 canvas suits Twitter and LinkedIn; a blog header is usually fine at 2x of its display width.",
	},
	{
		q: "Can I keep a transparent background?",
		a: "Yes. Set the backdrop to transparent and export as PNG. JPG has no alpha channel, so it will fill the transparency with white.",
	},
	{
		q: "Can I save a look and reuse it?",
		a: "Templates give you a one-click starting point, and your last settings persist in the browser between visits. There are no named custom presets yet.",
	},
	{
		q: "Why does my text look soft in the export?",
		a: "Almost always because the source screenshot was captured at 1x. The editor cannot invent detail that was not captured, so grab the shot on a retina display or at 2x, then export at 2x or higher.",
	},
	{
		q: "Can I edit several screenshots at once?",
		a: "Not in a batch. It is one image at a time, which keeps the editor simple. For a set with a shared look, pick a template and it will apply the same treatment to each.",
	},
	{
		q: "How is this different from the Recast desktop app?",
		a: "This makes a still screenshot presentable. The desktop app records your screen and turns the recording into a polished video, with auto-zoom, silence trimming and captions. Different jobs.",
	},
	{
		q: "Is there a file size or resolution limit?",
		a: "No hard cap, but the image is held in your browser memory while you work. Very large captures, above roughly 8000 pixels on a side, can get sluggish on a laptop.",
	},
];

/**
 * Structured data for the landing page: the app, its place in the site, and
 * the FAQ.
 *
 * `SoftwareApplication` and `BreadcrumbList` are the two that still earn a
 * richer result. `FAQPage` is kept because it is valid and other engines read
 * it, but Google narrowed FAQ rich results to health and government sites in
 * 2023.
 */
export function buildEditorJsonLd(origin = ""): string {
	const url = `${origin}/tools/screenshot-editor`;

	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "SoftwareApplication",
			"@id": `${url}#app`,
			name: EDITOR_TITLE,
			url,
			applicationCategory: "DesignApplication",
			applicationSubCategory: "Image Editor",
			operatingSystem: "Web",
			browserRequirements: "Requires a modern browser. No plugin or install.",
			permissions: "none",
			isAccessibleForFree: true,
			featureList: [
				"Gradient, mesh, pattern and image backdrops",
				"Browser and device mockup frames",
				"3D tilt and perspective",
				"Shadows, borders and glass styles",
				"Text and annotations",
				"Colour adjustments",
				"Motion presets with MP4 export",
				"PNG and JPG export at up to 4x",
			],
			description: EDITOR_DESCRIPTION,
			offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
			publisher: { "@type": "Organization", name: "Recast", url: origin || undefined },
		},
		{
			"@context": "https://schema.org",
			"@type": "BreadcrumbList",
			itemListElement: [
				{ "@type": "ListItem", position: 1, name: "Home", item: origin || undefined },
				{ "@type": "ListItem", position: 2, name: "Tools", item: `${origin}/tools` },
				{ "@type": "ListItem", position: 3, name: "Screenshot editor", item: url },
			],
		},
		{
			"@context": "https://schema.org",
			"@type": "FAQPage",
			"@id": `${url}#faq`,
			mainEntity: EDITOR_FAQ.map((f) => ({
				"@type": "Question",
				name: f.q,
				acceptedAnswer: { "@type": "Answer", text: f.a },
			})),
		},
	]);
}
