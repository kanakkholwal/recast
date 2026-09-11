/**
 * Font bytes for the engine's shaper: captions, composition titles, and the text
 * annotations the headless export draws.
 *
 * The DOM takes a `FontFace` and a woff2; rustybuzz takes neither. It needs the
 * TTF itself, which the host caches natively (Google serves TTF to an older UA,
 * woff2 to a modern one, so these are two different downloads of one family).
 * An INSTALLED family is never downloadable at all, so the host reads it from
 * the machine's own font database instead. A browser can do neither, and null
 * there means the DOM draws that text.
 */

import { getEditorServices } from "../editor/services";
import { googleFamilyFromStack, isGoogleFont } from "./google-fonts";

export interface EngineFont {
	/** Family plus weight; the engine re-uploads only when this changes. */
	key: string;
	data: Uint8Array;
}

// Successes only: a failure here is usually the first-use download, and caching it would strand the font.
const cache = new Map<string, EngineFont>();

/** Resolve a CSS stack to bytes the engine can shape, or null. The engine has no bundled face, so
 *  null means the DOM draws that text. Main thread only: it goes through the host's asset service. */
export async function resolveEngineFont(stack: string, weight = 400): Promise<EngineFont | null> {
	const host = getEditorServices().assets?.engineFontBytes;
	if (host) return hostEngineFont(host, stack, weight);
	const google = googleFamilyFromStack(stack);
	const downloadable = google !== null && isGoogleFont(google);
	const family = downloadable ? google : firstFamily(stack);
	if (!family) return null;
	const key = `${family}:${weight}`;
	const hit = cache.get(key);
	if (hit) return hit;

	const resolved = downloadable
		? await fetchEngineFont(family, weight, key)
		: await installedEngineFont(family, weight, key);
	if (resolved) cache.set(key, resolved);
	return resolved;
}

/** The desktop path: one host resolver shared with the native export, so both shape the same bytes. */
async function hostEngineFont(
	host: (stack: string, weight: number) => Promise<Uint8Array | null>,
	stack: string,
	weight: number,
): Promise<EngineFont | null> {
	const key = `host:${firstFamily(stack)}:${weight}`;
	const hit = cache.get(key);
	if (hit) return hit;
	try {
		const data = await host(stack, weight);
		if (!data) return null;
		const font = { key, data };
		cache.set(key, font);
		return font;
	} catch (err) {
		console.warn(`engine font unavailable: ${key}`, err);
		return null;
	}
}

/** The first name of a CSS stack, unquoted, which is what a font database matches. */
function firstFamily(stack: string): string {
	return (stack.split(",")[0] ?? stack).trim().replace(/^['"]|['"]$/g, "");
}

async function installedEngineFont(
	family: string,
	weight: number,
	key: string,
): Promise<EngineFont | null> {
	try {
		const data = await getEditorServices().assets?.installedFontBytes?.(family, weight);
		return data ? { key, data } : null;
	} catch (err) {
		console.warn(`engine font unavailable: ${key}`, err);
		return null;
	}
}

async function fetchEngineFont(
	family: string,
	weight: number,
	key: string,
): Promise<EngineFont | null> {
	try {
		const services = getEditorServices();
		const path = await services.assets?.captionFontFile?.(family, weight);
		if (!path) return null;
		const res = await fetch(services.resolveAssetUrl(path));
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		return { key, data: new Uint8Array(await res.arrayBuffer()) };
	} catch (err) {
		console.warn(`engine caption font unavailable: ${key}`, err);
		return null;
	}
}
