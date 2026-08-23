/** Font library transcribed from screenshot-studio (lib/constants/fonts.ts).
 * The reference loads these as Next.js CSS-variable fonts; this offline-first
 * port resolves to each font's own fallback stack instead, so fonts present on
 * the system render and the rest degrade to system-ui. Overlays store the font
 * `id`, matching upstream serialization. */

export type FontCategory =
	| "system"
	| "sans-serif"
	| "serif"
	| "display"
	| "handwriting"
	| "monospace";

export interface FontFamily {
	id: string;
	name: string;
	category: FontCategory;
	fallback: string;
	weights: string[];
}

/** 32 families across 6 categories. */
export const FONT_FAMILIES: FontFamily[] = [
	{
		id: "sf-pro-display",
		name: "SF Pro Display",
		category: "sans-serif",
		fallback: '"SF Pro Display", -apple-system, BlinkMacSystemFont, system-ui, sans-serif',
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "system",
		name: "System Default",
		category: "system",
		fallback: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
		weights: ["normal", "bold"],
	},
	{
		id: "arial",
		name: "Arial",
		category: "system",
		fallback: "Arial, Helvetica, sans-serif",
		weights: ["normal", "bold"],
	},
	{
		id: "inter",
		name: "Inter",
		category: "sans-serif",
		fallback: "Inter, system-ui, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "geist",
		name: "Geist",
		category: "sans-serif",
		fallback: "Geist, system-ui, sans-serif",
		weights: ["normal", "500", "600", "bold"],
	},
	{
		id: "poppins",
		name: "Poppins",
		category: "sans-serif",
		fallback: "Poppins, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "space-grotesk",
		name: "Space Grotesk",
		category: "sans-serif",
		fallback: "Space Grotesk, sans-serif",
		weights: ["300", "normal", "500", "600", "bold"],
	},
	{
		id: "outfit",
		name: "Outfit",
		category: "sans-serif",
		fallback: "Outfit, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "plus-jakarta-sans",
		name: "Plus Jakarta Sans",
		category: "sans-serif",
		fallback: "Plus Jakarta Sans, sans-serif",
		weights: ["200", "300", "normal", "500", "600", "bold", "800"],
	},
	{
		id: "dm-sans",
		name: "DM Sans",
		category: "sans-serif",
		fallback: "DM Sans, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "sora",
		name: "Sora",
		category: "sans-serif",
		fallback: "Sora, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800"],
	},
	{
		id: "manrope",
		name: "Manrope",
		category: "sans-serif",
		fallback: "Manrope, sans-serif",
		weights: ["200", "300", "normal", "500", "600", "bold", "800"],
	},
	{
		id: "raleway",
		name: "Raleway",
		category: "sans-serif",
		fallback: "Raleway, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "montserrat",
		name: "Montserrat",
		category: "sans-serif",
		fallback: "Montserrat, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "lexend",
		name: "Lexend",
		category: "sans-serif",
		fallback: "Lexend, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "work-sans",
		name: "Work Sans",
		category: "sans-serif",
		fallback: "Work Sans, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "urbanist",
		name: "Urbanist",
		category: "sans-serif",
		fallback: "Urbanist, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "albert-sans",
		name: "Albert Sans",
		category: "sans-serif",
		fallback: "Albert Sans, sans-serif",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "oswald",
		name: "Oswald",
		category: "display",
		fallback: "Oswald, Impact, sans-serif",
		weights: ["200", "300", "normal", "500", "600", "bold"],
	},
	{
		id: "bebas-neue",
		name: "Bebas Neue",
		category: "display",
		fallback: "Bebas Neue, Impact, sans-serif",
		weights: ["normal"],
	},
	{
		id: "righteous",
		name: "Righteous",
		category: "display",
		fallback: "Righteous, cursive",
		weights: ["normal"],
	},
	{
		id: "playfair-display",
		name: "Playfair Display",
		category: "serif",
		fallback: "Playfair Display, Georgia, serif",
		weights: ["normal", "500", "600", "bold", "800", "900"],
	},
	{
		id: "lora",
		name: "Lora",
		category: "serif",
		fallback: "Lora, Georgia, serif",
		weights: ["normal", "500", "600", "bold"],
	},
	{
		id: "libre-baskerville",
		name: "Libre Baskerville",
		category: "serif",
		fallback: "Libre Baskerville, Georgia, serif",
		weights: ["normal", "bold"],
	},
	{
		id: "georgia",
		name: "Georgia",
		category: "serif",
		fallback: "Georgia, Times, serif",
		weights: ["normal", "bold"],
	},
	{
		id: "caveat",
		name: "Caveat",
		category: "handwriting",
		fallback: "Caveat, cursive",
		weights: ["normal", "500", "600", "bold"],
	},
	{
		id: "pacifico",
		name: "Pacifico",
		category: "handwriting",
		fallback: "Pacifico, cursive",
		weights: ["normal"],
	},
	{
		id: "dancing-script",
		name: "Dancing Script",
		category: "handwriting",
		fallback: "Dancing Script, cursive",
		weights: ["normal", "500", "600", "bold"],
	},
	{
		id: "geist-mono",
		name: "Geist Mono",
		category: "monospace",
		fallback: "Geist Mono, monospace",
		weights: ["normal", "500", "600", "bold"],
	},
	{
		id: "jetbrains-mono",
		name: "JetBrains Mono",
		category: "monospace",
		fallback: "JetBrains Mono, monospace",
		weights: ["100", "200", "300", "normal", "500", "600", "bold", "800"],
	},
	{
		id: "fira-code",
		name: "Fira Code",
		category: "monospace",
		fallback: "Fira Code, monospace",
		weights: ["300", "normal", "500", "600", "bold"],
	},
	{
		id: "courier",
		name: "Courier New",
		category: "monospace",
		fallback: "Courier New, Courier, monospace",
		weights: ["normal", "bold"],
	},
];

export const FONT_CATEGORY_LABELS: Record<FontCategory, string> = {
	"sans-serif": "Modern Sans-Serif",
	display: "Display & Headlines",
	serif: "Elegant Serif",
	handwriting: "Handwriting & Script",
	monospace: "Monospace & Code",
	system: "System Fonts",
};

export const DEFAULT_FONT_ID = "inter";

export function fontById(id: string): FontFamily | undefined {
	return FONT_FAMILIES.find((f) => f.id === id);
}

/** Resolve a font id to a CSS font-family stack (fallback-only; no loaded vars). */
export function fontCss(id: string): string {
	return fontById(id)?.fallback ?? FONT_FAMILIES[0].fallback;
}

export function fontWeights(id: string): string[] {
	return fontById(id)?.weights ?? ["normal", "bold"];
}

export function fontsByCategory(category: FontCategory): FontFamily[] {
	return FONT_FAMILIES.filter((f) => f.category === category);
}
