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
];

/** SoftwareApplication + FAQPage JSON-LD for the landing page. */
export function buildEditorJsonLd(): string {
	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "SoftwareApplication",
			name: EDITOR_TITLE,
			applicationCategory: "DesignApplication",
			operatingSystem: "Web",
			offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
			description: EDITOR_DESCRIPTION,
		},
		{
			"@context": "https://schema.org",
			"@type": "FAQPage",
			mainEntity: EDITOR_FAQ.map((f) => ({
				"@type": "Question",
				name: f.q,
				acceptedAnswer: { "@type": "Answer", text: f.a },
			})),
		},
	]);
}
