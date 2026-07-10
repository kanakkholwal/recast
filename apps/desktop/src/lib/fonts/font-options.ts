// Combined font catalog (system + Google) shared by the caption and annotation
// font pickers, plus the on-demand loader hook. One source of truth so both
// pickers (and any future one) stay in sync.
import {
	GOOGLE_FONTS,
	googleFamilyFromStack,
	googleFontStack,
	loadGoogleFont,
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
		ALL_FONTS.find((f) => f.value === value)?.label ??
		googleFamilyFromStack(value) ??
		"Custom"
	);
}

/** Fetch + register the font if it's a Google font (no-op for system fonts). */
export function ensureFontLoaded(value: string, weight = 400): void {
	if (isSystem(value)) return;
	const family = googleFamilyFromStack(value);
	if (family) void loadGoogleFont(family, weight);
}
