import type { StoredCursorId } from "../../stores/editor-store.svelte";
import { cursorSpriteHotspot, resolveCursorSprite } from "../registry";
import type { CursorSpriteUpload } from "./engine-driver";

/** Sprite hotspots are authored in a 64-unit box. */
const SPRITE_UNITS = 64;

/** Rasterised edge in device pixels. Fixed rather than derived from the
 *  on-screen size: the sprite is scaled by the sampler, and re-rasterising on
 *  every zoom step would cost more than the extra texels. */
const RASTER_PX = 128;

const SLOTS = ["rest", "press", "rightPress", "drag"] as const;

/**
 * Rasterises a cursor style into the four sprite slots. Returns an empty list
 * for a style with no sprite, which is what puts the engine back on the dot.
 */
export async function loadCursorSprites(
	styleId: StoredCursorId,
	resolveDataUrl: (id: StoredCursorId, state: (typeof SLOTS)[number]) => string | null,
): Promise<CursorSpriteUpload[]> {
	const style = resolveCursorSprite(styleId);
	if (!style) return [];

	const uploads = await Promise.all(
		SLOTS.map(async (slot) => {
			const src = resolveDataUrl(styleId, slot);
			if (!src) return null;
			const image = await rasterise(src);
			if (!image) return null;
			const hotspot = cursorSpriteHotspot(style, slot);
			return {
				slot,
				image,
				hotspot: [hotspot.x / SPRITE_UNITS, hotspot.y / SPRITE_UNITS] as [number, number],
			};
		}),
	);
	return uploads.filter((upload): upload is CursorSpriteUpload => upload !== null);
}

async function rasterise(src: string): Promise<ImageBitmap | null> {
	try {
		const img = new Image();
		img.src = src;
		await img.decode();
		return await createImageBitmap(img, {
			resizeWidth: RASTER_PX,
			resizeHeight: RASTER_PX,
			resizeQuality: "high",
		});
	} catch (err) {
		console.warn("cursor sprite could not be rasterised:", err);
		return null;
	}
}
