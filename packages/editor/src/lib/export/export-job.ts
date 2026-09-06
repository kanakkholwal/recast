/**
 * Serializable export job — the handoff between the main thread (which snapshots
 * the scene and rasterises DOM-bound assets to bitmaps) and the render worker
 * (which owns the OffscreenCanvas, the engine and the MediaBunny loop). Every
 * field is either structured-cloneable or a transferable `ImageBitmap`, so the
 * whole job crosses `postMessage` with no closures.
 *
 * The engine evaluates the scene itself, so this carries the scene plus the
 * assets wasm cannot fetch: bitmaps, sprites, and the caption font's bytes.
 */

import type { MediaRef } from "@recast/media";
import type { CursorSpriteUpload } from "../playback/engine-driver";
import type { TimeMap } from "../timeline/time-map";
import type { ExportQuality } from "./browser-export-plan";
import type { CameraExportInputs } from "./offscreen-export";

export interface ExportJob {
	/** The scene the engine evaluates: `store.toRenderState()`. */
	scene: unknown;
	/** Output-to-original mapping (cuts and speed). */
	timeMap: TimeMap;
	/** Total output duration after cuts/speed (seconds). */
	outputDurationSec: number;
	fps: number;
	/** Quality tier ONLY — the encoder config carries a branded MediaBunny
	 *  `Quality` object that does not survive `postMessage`, so the worker
	 *  rebuilds it. */
	quality: ExportQuality;
	/** Source video URL — the worker opens its own decoder on it. */
	videoUrl: MediaRef | string;
	/** Source dimensions, which set the composition's aspect. */
	sourceWidth: number;
	sourceHeight: number;
	/** Composited output size (px), the composition's native size. */
	width: number;
	height: number;
	/** Decoded image/wallpaper background, or null for colour and gradient. */
	backgroundImage: ImageBitmap | null;
	cursorSprites: CursorSpriteUpload[];
	/** Smoothed cursor samples and idle spans, or null when there is no track. */
	cursorTrack: unknown | null;
	/** Word timings for burned captions, or null when they are not burned in. */
	captionTrack: unknown | null;
	/** TTF bytes for the caption face; null falls back to the bundled one. */
	captionFont: Uint8Array | null;
	/** Decoded assets for image annotations, by path. */
	annotationImages: Array<[string, ImageBitmap]>;
	camera: CameraExportInputs | null;
}

/** Every distinct bitmap the job owns. Deduped — the cursor sprite fallbacks
 *  (drag/rightPress → press → rest) can share one bitmap. */
function jobBitmaps(job: ExportJob): ImageBitmap[] {
	const seen = new Set<ImageBitmap>();
	if (job.backgroundImage) seen.add(job.backgroundImage);
	for (const s of job.cursorSprites) seen.add(s.image);
	for (const [, bmp] of job.annotationImages) seen.add(bmp);
	return [...seen];
}

/** What `postMessage` should move rather than copy. The font bytes go too: a
 *  multi-megabyte TTF is worth not cloning. */
export function collectTransferables(job: ExportJob): Transferable[] {
	const out: Transferable[] = [...jobBitmaps(job)];
	if (job.captionFont) out.push(job.captionFont.buffer);
	return out;
}

/** Release every bitmap the job owns. Idempotent, so an early throw cannot leak. */
export function closeJobBitmaps(job: ExportJob): void {
	for (const bmp of jobBitmaps(job)) bmp.close();
}
