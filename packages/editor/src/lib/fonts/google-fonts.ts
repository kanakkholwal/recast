// Shared on-demand Google Fonts loader, used by captions (and reusable by
// annotations). A font is fetched + cached on device by the host's asset service
// on first use, then registered with the document via the FontFace API so any
// element using that family renders it. The full searchable catalog can come
// later; this curated set covers the common display/caption choices.
import { getEditorServices } from "$lib/editor/services";

/** Curated, searchable set of popular Google Fonts offered in pickers. */
export const GOOGLE_FONTS = [
	"Inter",
	"Roboto",
	"Roboto Condensed",
	"Roboto Slab",
	"Open Sans",
	"Montserrat",
	"Poppins",
	"Lato",
	"Nunito",
	"Nunito Sans",
	"Work Sans",
	"Raleway",
	"Rubik",
	"Mulish",
	"Manrope",
	"DM Sans",
	"DM Serif Display",
	"Figtree",
	"Outfit",
	"Sora",
	"Space Grotesk",
	"Plus Jakarta Sans",
	"Oswald",
	"Bebas Neue",
	"Anton",
	"Archivo",
	"Archivo Black",
	"Teko",
	"Barlow",
	"Kanit",
	"Playfair Display",
	"Merriweather",
	"Lora",
	"PT Serif",
	"Source Serif 4",
	"Caveat",
	"Pacifico",
	"Dancing Script",
	"Permanent Marker",
	"Fredoka",
	"Comfortaa",
	"Josefin Sans",
	"Quicksand",
	// Sans
	"Source Sans 3",
	"IBM Plex Sans",
	"Libre Franklin",
	"Karla",
	"Jost",
	"Urbanist",
	"Lexend",
	"Red Hat Display",
	"Be Vietnam Pro",
	"Hanken Grotesk",
	"Albert Sans",
	"Heebo",
	"PT Sans",
	"Cabin",
	// Serif
	"EB Garamond",
	"Cormorant Garamond",
	"Bitter",
	"Libre Baskerville",
	"Spectral",
	"Zilla Slab",
	"IBM Plex Serif",
	"Domine",
	"Noto Serif",
	// Display
	"Abril Fatface",
	"Lobster",
	"Alfa Slab One",
	"Righteous",
	"Fjalla One",
	"Staatliches",
	"Bungee",
	"Russo One",
	"Yanone Kaffeesatz",
	// Handwriting
	"Satisfy",
	"Great Vibes",
	"Shadows Into Light",
	"Indie Flower",
	"Sacramento",
	"Kalam",
	// Mono
	"JetBrains Mono",
	"Fira Code",
	"Space Mono",
	"Source Code Pro",
	"IBM Plex Mono",
] as const;

/** Generic CSS fallback for a Google Fonts category, used while the webfont
 *  is still loading (or if it fails). */
export function fallbackForCategory(category?: string): string {
	switch (category) {
		case "Serif":
			return "serif";
		case "Monospace":
			return "monospace";
		case "Handwriting":
			return "cursive";
		case "Display":
			return "'Arial Narrow', sans-serif";
		default:
			return "sans-serif";
	}
}

/** A CSS font-family stack for a Google font, e.g. `'Poppins', sans-serif`. */
export const googleFontStack = (family: string, category?: string) =>
	`'${family}', ${fallbackForCategory(category)}`;

/** The Google family quoted at the head of a stack, e.g.
 *  `'Playfair Display', serif` → `Playfair Display`. Returns null for stacks
 *  that don't start with a quoted family (the system fonts). */
export function googleFamilyFromStack(stack: string): string | null {
	const m = stack.match(/^\s*'([^']+)'/);
	return m ? m[1] : null;
}

/** Whether `family` is a fetchable Google font. Fontsource-bundled fonts (Geist,
 *  etc.) also sit behind a quoted family but are NOT on Google — don't fetch them. */
export function isGoogleFont(family: string): boolean {
	return (GOOGLE_FONTS as readonly string[]).includes(family);
}

const loaded = new Set<string>();

/** Google's own woff2 endpoint, for hosts with no on-device font cache. The css2
 *  API needs a browser-shaped UA to return woff2, which is why it isn't the
 *  desktop path. */
function googleCdnFontUrl(family: string, weight: number): string {
	const name = family.replace(/ /g, "+");
	return `https://fonts.googleapis.com/css2?family=${name}:wght@${weight}&display=swap`;
}

/**
 * Resolve a loadable URL for `family` at `weight`, or null on failure. A host
 * with an asset service caches the file on device; without one we fetch Google's
 * CDN directly. Main-thread only — a worker gets the URL passed in instead.
 */
export async function resolveGoogleFontUrl(family: string, weight = 400): Promise<string | null> {
	try {
		const services = getEditorServices();
		if (!services.assets) return googleCdnFontUrl(family, weight);
		return services.resolveAssetUrl(await services.assets.googleFont(family, weight));
	} catch (e) {
		console.warn(`Google font resolve failed: ${family} ${weight}`, e);
		return null;
	}
}

/**
 * Ensure `family` at `weight` is downloaded + registered with the document.
 * Idempotent per family+weight; failures are swallowed (the text just falls
 * back to the next family in the stack).
 */
export async function loadGoogleFont(family: string, weight = 400): Promise<void> {
	const key = `${family}:${weight}`;
	if (loaded.has(key)) return;
	loaded.add(key);
	const url = await resolveGoogleFontUrl(family, weight);
	if (!url) {
		loaded.delete(key);
		return;
	}
	try {
		const face = new FontFace(family, `url("${url}")`, { weight: String(weight) });
		await face.load();
		// `FontFaceSet.add` is missing from the lib typings in this config.
		(document.fonts as unknown as { add(f: FontFace): void }).add(face);
	} catch (e) {
		loaded.delete(key);
		console.warn(`Google font load failed: ${family} ${weight}`, e);
	}
}
