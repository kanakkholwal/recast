/**
 * Resolve a scene's `backgroundValue` to a loadable URL, shared by the preview
 * (WebGL texture upload) and the offline export renderer. Handles extension
 * wallpapers (`ext:`), downloadable assets (`asset:`), data/http/root URLs, and
 * raw filesystem paths (→ Tauri asset protocol). Returns "" when there's no
 * image to load (colour/gradient, or an asset not cached yet).
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveAsset } from "$lib/assets";
import { resolveBackgroundWireValue } from "$lib/registry";
import { assetsStore } from "$lib/stores/assets-store.svelte";

export async function resolveBackgroundSrc(value: string): Promise<string> {
	if (!value) return "";
	if (value.startsWith("ext:")) {
		const wire = resolveBackgroundWireValue(value);
		if (!wire || wire.startsWith("#")) return "";
		return convertFileSrc(wire);
	}
	// Defensive: keep gradient/colour values away from convertFileSrc, since a
	// stray write leaving a CSS gradient here while type briefly reads "image"
	// would otherwise log a bogus "File does not exist" via the asset protocol.
	if (value.includes("gradient(") || value.startsWith("#")) return "";
	if (value.startsWith("asset:") && !value.startsWith("asset://")) {
		const id = value.slice("asset:".length);
		const cached = await resolveAsset(id);
		if (cached) return convertFileSrc(cached);
		const thumb = assetsStore.thumbPaths[id];
		if (thumb) return convertFileSrc(thumb);
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
	return convertFileSrc(value);
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
