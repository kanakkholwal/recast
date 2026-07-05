/**
 * Pure helpers for ToolPage: SEO schema, control partitioning, worker-option
 * assembly, output-kind classification, and the stage machine. No Svelte, no
 * DOM — the component keeps the reactive state, effects, and conversion glue.
 */

import type { ToolControl, ToolDef } from "$lib/tools/registry";
import type { ToolOptions } from "$lib/tools/worker-protocol";

// Option keys whose <select> values are strings in the DOM but must reach the
// worker as numbers (bitrate presets, dimensions chosen from a menu, etc.).
export const NUMERIC_KEYS = [
	"width",
	"height",
	"fps",
	"startSec",
	"endSec",
	"frameCount",
	"videoBitrate",
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
		opts[c.key] = NUMERIC_KEYS.includes(c.key)
			? Number(selectValues[c.key])
			: selectValues[c.key];
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

/** SoftwareApplication + FAQPage JSON-LD for the tool's landing page. */
export function buildToolJsonLd(tool: ToolDef): string {
	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "SoftwareApplication",
			name: tool.title,
			applicationCategory: "MultimediaApplication",
			operatingSystem: "Web",
			offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
			description: tool.description,
		},
		{
			"@context": "https://schema.org",
			"@type": "FAQPage",
			mainEntity: tool.faq.map((f) => ({
				"@type": "Question",
				name: f.q,
				acceptedAnswer: { "@type": "Answer", text: f.a },
			})),
		},
	]);
}
