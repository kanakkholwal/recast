/**
 * Resolve a scene's `backgroundValue` to a loadable URL, shared by the preview
 * (WebGL texture upload) and the offline export renderer. Handles extension
 * wallpapers (`ext:`), downloadable assets (`asset:`), data/http/root URLs, and
 * raw asset refs (→ the host's resolver). Returns "" when there's no image to
 * load (colour/gradient, or an asset not cached yet).
 */

import { resolveAsset } from "../lib/assets";
import { getEditorServices } from "../lib/editor/services";
import { resolveBackgroundWireValue } from "../lib/registry";
import { assetsStore } from "../stores/assets-store.svelte";

export async function resolveBackgroundSrc(value: string): Promise<string> {
	if (!value) return "";
	const resolve = getEditorServices().resolveAssetUrl;
	if (value.startsWith("ext:")) {
		const wire = resolveBackgroundWireValue(value);
		if (!wire || wire.startsWith("#")) return "";
		return resolve(wire);
	}
	// Defensive: a stray CSS gradient reaching the resolver while type briefly reads 'image' logs a bogus missing-file error.
	if (value.includes("gradient(") || value.startsWith("#")) return "";
	if (value.startsWith("asset:") && !value.startsWith("asset://")) {
		const id = value.slice("asset:".length);
		const cached = await resolveAsset(id);
		if (cached) return resolve(cached);
		const thumb = assetsStore.thumbPaths[id];
		if (thumb) return resolve(thumb);
		return "";
	}
	if (
		value.startsWith("data:") ||
		value.startsWith("http://") ||
		value.startsWith("https://") ||
		value.startsWith("asset://") ||
		value.startsWith("/")
	) {
		return value;
	}
	return resolve(value);
}

/** Decode a scene background to an `ImageBitmap`, or null when it isn't an image
 *  type / can't be loaded. Used by the export renderer to build the bg texture. */
export async function loadBackgroundBitmap(
	type: string,
	value: string,
): Promise<ImageBitmap | null> {
	if (type !== "wallpaper" && type !== "image") return null;
	const src = await resolveBackgroundSrc(value);
	if (!src) return null;
	const img = new Image();
	img.crossOrigin = "anonymous";
	img.src = src;
	await img.decode();
	return createImageBitmap(img);
}
