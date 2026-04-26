/**
 * Hybrid-raster export path for the SVG cursor sprite.
 *
 * Same pattern as `rasterize-text.ts`: the Rust export pipeline can decode
 * `data:image/png;base64,…` URLs (via `build_image_cache` in cursor_export.rs)
 * but has no SVG renderer. So at export start we rasterize each cursor sprite
 * (rest + optional pressed state) once via an offscreen canvas and ship the
 * resulting PNG data URLs through `RenderState`. Rust composites them per
 * frame at the cursor sample's canvas position.
 *
 * Rasterizing once per export amortises the SVG decode cost across thousands
 * of frames, and reuses the existing PNG decode + alpha-blend path on the
 * Rust side instead of bringing in a new sprite codec.
 */

import { CURSOR_STYLES } from "$lib/cursor/styles";
import type { CursorStyleId } from "$lib/stores/editor-store.svelte";

export interface CursorSpriteBundle {
	/** PNG data URL of the resting sprite. Always populated. */
	rest: string;
	/** PNG data URL of the pressed sprite. Falls back to `rest` when the
	 *  active style has no pressed-state variant. */
	press: string;
	/** Hotspot in 0..1 sprite UV. Used by Rust to anchor each sprite's
	 *  click point to the cursor sample position regardless of size. */
	restHotspot: [number, number];
	pressHotspot: [number, number];
	/** Sprite render size in source pixels — Rust composites at this size,
	 *  scaled by the canvas pixel ratio (the same factor `cursorRadiusCanvas`
	 *  uses). Cached so the rest of the pipeline can label by size. */
	pixelSize: number;
}

// Re-rasterizing the same sprite twice in a row is wasted work — the SVG
// strings and the requested size don't change between consecutive exports
// of the same project. Key by `${style}:${size}`.
const cache = new Map<string, CursorSpriteBundle>();

/**
 * Rasterize the active cursor style's sprites at the requested source-pixel
 * size. Returns `null` for the soft-dot style — Rust draws that itself.
 */
export async function rasterizeCursorSprites(
	styleId: CursorStyleId,
	pixelSize: number,
): Promise<CursorSpriteBundle | null> {
	if (styleId === "dot") return null;
	const px = Math.max(8, Math.round(pixelSize));
	const cacheKey = `${styleId}:${px}`;
	const hit = cache.get(cacheKey);
	if (hit) return hit;

	const style = CURSOR_STYLES.find((s) => s.id === styleId);
	if (!style) return null;

	const rest = await renderSvgToDataUrl(style.svg, px);
	if (!rest) return null;

	let press = rest;
	if (style.pressedSvg) {
		const pressed = await renderSvgToDataUrl(style.pressedSvg, px);
		if (pressed) press = pressed;
	}

	const bundle: CursorSpriteBundle = {
		rest,
		press,
		restHotspot: [style.hotspot.x / 64, style.hotspot.y / 64],
		pressHotspot: style.pressedHotspot
			? [style.pressedHotspot.x / 64, style.pressedHotspot.y / 64]
			: [style.hotspot.x / 64, style.hotspot.y / 64],
		pixelSize: px,
	};
	cache.set(cacheKey, bundle);
	return bundle;
}

async function renderSvgToDataUrl(
	svg: string,
	pixelSize: number,
): Promise<string | null> {
	const blob = new Blob([svg.trim()], { type: "image/svg+xml" });
	const url = URL.createObjectURL(blob);
	try {
		const img = new Image();
		img.decoding = "async";
		const loaded = new Promise<HTMLImageElement>((resolve, reject) => {
			img.onload = () => resolve(img);
			img.onerror = () =>
				reject(new Error("rasterize-cursor: failed to decode SVG sprite"));
		});
		img.src = url;
		await loaded;

		const canvas = document.createElement("canvas");
		canvas.width = pixelSize;
		canvas.height = pixelSize;
		const ctx = canvas.getContext("2d");
		if (!ctx) return null;
		ctx.clearRect(0, 0, pixelSize, pixelSize);
		ctx.drawImage(img, 0, 0, pixelSize, pixelSize);
		return canvas.toDataURL("image/png");
	} catch (err) {
		console.warn("rasterize-cursor:", err);
		return null;
	} finally {
		URL.revokeObjectURL(url);
	}
}