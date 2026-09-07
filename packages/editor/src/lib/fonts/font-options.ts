// One combined catalog plus the on-demand loader hook, so the caption and annotation pickers stay in sync.
import {
	GOOGLE_FONTS,
	googleFamilyFromStack,
	googleFontStack,
	isGoogleFont,
	loadGoogleFont,
	resolveGoogleFontUrl,
} from "./google-fonts";

export interface FontOption {
	label: string;
	/** CSS font-family stack. */
	value: string;
}

/** Always-available system / web-safe stacks. */
export const SYSTEM_FONTS: FontOption[] = [
	{ label: "Sans", value: "system-ui, sans-serif" },
	{ label: "Serif", value: "Georgia, 'Times New Roman', serif" },
	{ label: "Mono", value: "'Courier New', monospace" },
	{ label: "Impact", value: "Impact, 'Arial Narrow Bold', sans-serif" },
];

export const GOOGLE_FONT_OPTIONS: FontOption[] = GOOGLE_FONTS.map((f) => ({
	label: f,
	value: googleFontStack(f),
}));

const ALL_FONTS: FontOption[] = [...SYSTEM_FONTS, ...GOOGLE_FONT_OPTIONS];

const isSystem = (value: string) => SYSTEM_FONTS.some((f) => f.value === value);

/** Human label for a stored font value (family name for Google fonts). */
export function fontLabel(value: string): string {
	return (
		ALL_FONTS.find((f) => f.value === value)?.label ?? googleFamilyFromStack(value) ?? "Custom"
	);
}

/** Fetch + register the font if it's a Google font. No-op for system stacks and
 *  fontsource-bundled fonts (Inter, etc.) — those are already in the document. */
export function ensureFontLoaded(value: string, weight = 400): void {
	if (isSystem(value)) return;
	const family = googleFamilyFromStack(value);
	if (family && isGoogleFont(family)) void loadGoogleFont(family, weight);
}

/** Where a burned-caption export must render to get `value` at the right font.
 *  `worker` with no font: a system/generic stack OffscreenCanvas already has.
 *  `worker` with a font: a Google font, resolved to a URL the worker registers.
 *  `main`: a fontsource-bundled (or otherwise document-only) font the worker's
 *  `self.fonts` can't see — render on the main thread, where `document.fonts` has it. */
export type CaptionFontPlan =
	| { where: "worker"; font?: { family: string; url: string; weight: number } }
	| { where: "main" };

export async function planCaptionFont(value: string, weight = 400): Promise<CaptionFontPlan> {
	if (isSystem(value)) return { where: "worker" };
	const family = googleFamilyFromStack(value);
	if (!family) return { where: "worker" }; // generic stack (ui-serif, ui-monospace, …)
	if (isGoogleFont(family)) {
		const url = await resolveGoogleFontUrl(family, weight);
		return url ? { where: "worker", font: { family, url, weight } } : { where: "main" };
	}
	return { where: "main" }; // fontsource-bundled or custom — only document.fonts has it
}
