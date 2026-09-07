/**
 * Pure helpers for ToolPage: SEO schema, control partitioning, worker-option
 * assembly, output-kind classification, and the stage machine. No Svelte, no
 * DOM — the component keeps the reactive state, effects, and conversion glue.
 */

import type { ToolControl, ToolDef } from "$lib/tools/registry";
import type { ToolOptions } from "$lib/tools/worker-protocol";

// Option keys whose select values are DOM strings but must reach the worker as numbers.
export const NUMERIC_KEYS = [
	"width",
	"height",
	"fps",
	"startSec",
	"endSec",
	"frameCount",
	"videoBitrate",
	"gifColors",
];

export const selectControlsOf = (tool: ToolDef): ToolControl[] =>
	tool.controls?.filter((c) => c.type === "select") ?? [];

export const numberControlsOf = (tool: ToolDef): ToolControl[] =>
	tool.controls?.filter((c) => c.type === "number") ?? [];

/** Assemble the worker option bag from fixed options plus the user's picks. */
export function buildToolOptions(
	tool: ToolDef,
	selectControls: ToolControl[],
	numberControls: ToolControl[],
	selectValues: Record<string, string>,
	numberValues: Record<string, number>,
): ToolOptions {
	const opts: Record<string, unknown> = { ...(tool.fixedOptions ?? {}) };
	for (const c of selectControls) {
		opts[c.key] = NUMERIC_KEYS.includes(c.key) ? Number(selectValues[c.key]) : selectValues[c.key];
	}
	for (const c of numberControls) opts[c.key] = numberValues[c.key];
	return opts as ToolOptions;
}

export type OutputKind = "video" | "image" | "audio" | "file";

/** Classify a result MIME into the preview surface it should render in. */
export function outputKindFor(mime: string): OutputKind {
	if (!mime) return "file";
	if (mime.startsWith("video/")) return "video";
	if (mime === "image/gif") return "image";
	if (mime.startsWith("audio/")) return "audio";
	return "file";
}

export type ToolPhase = "blocked" | "processing" | "result" | "ready" | "select";

/** The single stage the UI is in, from highest-priority guard downward. */
export function resolvePhase(
	blocked: boolean,
	busy: boolean,
	hasResult: boolean,
	hasFile: boolean,
): ToolPhase {
	return blocked
		? "blocked"
		: busy
			? "processing"
			: hasResult
				? "result"
				: hasFile
					? "ready"
					: "select";
}

/**
 * Structured data for a tool page: the app itself, its place in the site
 * hierarchy, and the FAQ.
 *
 * `SoftwareApplication` and `BreadcrumbList` are the two that still earn a
 * richer result for a page like this. `FAQPage` stays because it is valid and
 * other engines read it, but Google restricted FAQ rich results to
 * authoritative health and government sites in 2023, so do not expect the
 * dropdowns to show in Google.
 */
export function buildToolJsonLd(tool: ToolDef, origin = ""): string {
	const url = `${origin}/tools/${tool.slug}`;
	const features = [
		"Runs entirely in the browser",
		"No file upload",
		"No account required",
		"No watermark",
		`Outputs ${tool.outputLabel}`,
	];

	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "SoftwareApplication",
			"@id": `${url}#app`,
			name: tool.title,
			url,
			applicationCategory: "MultimediaApplication",
			applicationSubCategory: "Video Converter",
			operatingSystem: "Web",
			browserRequirements: "Requires a browser with WebCodecs support, such as Chrome or Edge.",
			// No install step and nothing to grant: worth stating, since it is the differentiator against server-side converters.
			permissions: "none",
			isAccessibleForFree: true,
			featureList: features,
			description: tool.description,
			offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
			publisher: { "@type": "Organization", name: "Recast", url: origin || undefined },
		},
		{
			"@context": "https://schema.org",
			"@type": "BreadcrumbList",
			itemListElement: [
				{ "@type": "ListItem", position: 1, name: "Home", item: origin || undefined },
				{ "@type": "ListItem", position: 2, name: "Tools", item: `${origin}/tools` },
				{ "@type": "ListItem", position: 3, name: tool.title, item: url },
			],
		},
		{
			"@context": "https://schema.org",
			"@type": "FAQPage",
			"@id": `${url}#faq`,
			mainEntity: tool.faq.map((f) => ({
				"@type": "Question",
				name: f.q,
				acceptedAnswer: { "@type": "Answer", text: f.a },
			})),
		},
	]);
}

const ISSUE_BASE = "https://github.com/kanakkholwal/recast/issues/new";

/**
 * A pre-filled bug report for one tool. The template carries the facts we
 * always end up asking for, so a report arrives usable instead of as
 * "the converter is broken".
 */
export function buildIssueUrl(tool: ToolDef, browserInfo = ""): string {
	const body = [
		`**Tool:** ${tool.title} (\`/tools/${tool.slug}\`)`,
		"",
		"**What happened**",
		"",
		"",
		"**What I expected**",
		"",
		"",
		"**Input file**",
		"Format, resolution and rough length:",
		"",
		"**Settings used**",
		"",
		"",
		`**Browser:** ${browserInfo || "(please paste your browser and version)"}`,
		"",
		"<!-- These tools run entirely in your browser, so please do not attach",
		"     anything confidential. A description is usually enough. -->",
	].join("\n");

	const params = new URLSearchParams({
		title: `[tools] ${tool.title}: `,
		labels: "tools",
		body,
	});
	return `${ISSUE_BASE}?${params.toString()}`;
}
