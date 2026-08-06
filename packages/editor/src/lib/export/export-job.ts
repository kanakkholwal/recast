import type { MediaRef } from "@recast/media";
import type { AudioExportInputs } from "./offscreen-export";
/**
 * Serializable export job — the handoff contract between the main thread (which
 * snapshots the scene + rasterizes DOM-bound assets to bitmaps) and the render
 * worker (which owns the OffscreenCanvas + RenderCore + MediaBunny loop). Every
 * field is either structured-cloneable or a transferable `ImageBitmap`, so the
 * whole job crosses `postMessage` with zero closures — the worker rebuilds the
 * per-frame callbacks from this data using the same pure builders the preview
 * uses. Migration Phase 3 (export off the main thread).
 *
 * The per-frame draw fns (drawAnnotationLayerExport / drawCaptionLayerExport) are
 * already pure (2D ctx + data); only asset prep (images, cursor sprites, fonts)
 * is DOM-bound and stays main-thread, emitting the bitmaps carried here.
 */

import type { FrameInput } from "../../components/frame-params";
import type { TimeMap } from "../timeline/time-map";
import type { ExportQuality } from "./browser-export-plan";
import type { EditorStore } from "../../stores/editor-store.svelte";
import type { CursorSpriteSources } from "./cursor-overlay-export";
import type { AnnotationLayerInputs } from "./annotation-layer-export";
import type { CaptionLayerInputs } from "./caption-layer-export";
import type { CameraExportInputs } from "./offscreen-export";

/** Annotation layer as data: the pure inputs minus the runtime-only `getImage`
 *  (rebuilt from `images`) and `blur` (the worker's own composited framebuffer). */
export interface AnnotationJob extends Omit<AnnotationLayerInputs, "getImage" | "blur"> {
	/** Decoded image annotations by path — transferable, rebuilt into `getImage`. */
	images: Array<[string, ImageBitmap]>;
}

/** Caption layer as data — already fully serializable (transcript/style/timeMap/
 *  geometry). `font` is the resolved webfont the worker registers before paint
 *  (absent for system fonts). `mainThreadOnly` marks a fontsource/document-only
 *  font the worker can't see, so the whole render stays on the main thread. */
export type CaptionJob = CaptionLayerInputs & {
	font?: { family: string; url: string; weight: number };
	mainThreadOnly?: boolean;
};

/** Camera bubble as data: everything CameraExportInputs needs except the resolved
 *  `placementAt(t)` closure, which the worker rebuilds from the shared placement
 *  helpers. `url` is decoded worker-side (its own MediaBunny Input). */
export interface CameraJob extends Omit<CameraExportInputs, "placementAt"> {
	/** Camera keyframe/zoom-follow inputs, replayed to rebuild `placementAt`. */
	placement: CameraPlacementJob;
}

/** The store-sourced values behind `buildCameraInputs`'s `placementAt`. */
export interface CameraPlacementJob {
	defaultPlacement: EditorStore["cameraOverlay"]["defaultPlacement"];
	keyframes: EditorStore["cameraOverlay"]["keyframes"];
	keyframeEasing: EditorStore["cameraOverlay"]["keyframeEasing"];
	zoomFollow: boolean;
	focusEnabled: boolean;
	zoomRegions: EditorStore["zoomRegions"];
	zoomFollowDuration: number;
	zoomFollowEasing: EditorStore["cameraOverlay"]["zoomFollowEasing"];
	zoomFollowStrength: number;
	videoAspect: number;
}

export interface ExportJob {
	/** Static scene (every FrameInput field except the per-frame `playbackTime`). */
	base: Omit<FrameInput, "playbackTime">;
	/** Output↔original mapping (cuts + speed). */
	timeMap: TimeMap;
	/** Total output duration after cuts/speed (seconds). */
	outputDurationSec: number;
	fps: number;
	/** Quality tier ONLY — the encoder config carries a branded MediaBunny `Quality`
	 *  object that doesn't survive `postMessage`, so the worker rebuilds it. */
	quality: ExportQuality;
	/** Source video URL — the worker opens its own decoder on it. */
	videoUrl: MediaRef | string;
	/** Source audio to carry into the mux; omitted ⇒ a video-only mp4. */
	audio?: AudioExportInputs | null;
	/** Decoded image/wallpaper background (transferable), or null for colour/gradient. */
	backgroundImage: ImageBitmap | null;
	cursorSprites: CursorSpriteSources | null;
	camera: CameraJob | null;
	annotation: AnnotationJob | null;
	caption: CaptionJob | null;
}

/** Every distinct bitmap the job owns. Deduped — the cursor sprite fallbacks
 *  (drag/rightPress → press → rest) can share one bitmap. */
function jobBitmaps(job: ExportJob): ImageBitmap[] {
	const seen = new Set<ImageBitmap>();
	const add = (b: ImageBitmap | null | undefined) => {
		if (b) seen.add(b);
	};
	add(job.backgroundImage);
	if (job.cursorSprites) {
		add(job.cursorSprites.rest);
		add(job.cursorSprites.press);
		add(job.cursorSprites.rightPress);
		add(job.cursorSprites.drag);
	}
	if (job.annotation) for (const [, bmp] of job.annotation.images) add(bmp);
	return [...seen];
}

/** Bitmaps for `postMessage`'s transfer list, to move them zero-copy. */
export function collectTransferables(job: ExportJob): Transferable[] {
	return jobBitmaps(job);
}

/** Free the job's bitmaps. Call after a worker render (which consumed clones) so
 *  the main-thread originals don't leak; the main-thread path closes its own. */
export function closeJobBitmaps(job: ExportJob): void {
	for (const bmp of jobBitmaps(job)) bmp.close();
}
