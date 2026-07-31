/**
 * Export job consumer — turns a serialized {@link ExportJob} into encoded mp4
 * bytes by rebuilding the per-frame callbacks (cursor overlay, camera placement,
 * annotation + caption layers) from the job's plain data and driving the shared
 * offline renderer. Intentionally DOM-free: it runs on the main thread today and
 * moves verbatim into the render worker (Phase 3). The producer (build-export-job)
 * owns all the DOM-bound asset prep.
 */

import {
	renderTimelineToVideo,
	type BlurLayerEnv,
	type CameraExportInputs,
} from "./offscreen-export";
import { cursorOverlayFactory } from "./cursor-overlay-export";
import { drawAnnotationLayerExport } from "./annotation-layer-export";
import { drawCaptionLayerExport } from "./caption-layer-export";
import { makeExportFrameAt } from "./export-frame-input";
import {
	applyZoomFollow,
	cameraFollowScaleAt,
	cameraPlacementAt,
} from "../../components/editor/_components/camera-overlay.logic";
import type { ShapeImage } from "@recast/render";
import type { ExportJob, CameraJob } from "./export-job";

/** Runtime callbacks that can't cross into the job payload (they're live handles). */
export interface ExportRuntime {
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** Rebuild CameraExportInputs.placementAt from the serialized keyframe/zoom-follow
 *  inputs — the exact mirror of buildCameraInputs in the producer. */
function reconstructCamera(c: CameraJob): CameraExportInputs {
	const p = c.placement;
	return {
		url: c.url,
		geom: c.geom,
		shape: c.shape,
		cornerRadius: c.cornerRadius,
		mirror: c.mirror,
		placementAt: (t: number) => {
			const b = cameraPlacementAt(p.defaultPlacement, p.keyframes, t, p.keyframeEasing);
			if (!p.zoomFollow || !p.focusEnabled) return b;
			const zoom = cameraFollowScaleAt(p.zoomRegions, t, p.zoomFollowDuration, p.zoomFollowEasing);
			return applyZoomFollow(
				b,
				zoom,
				{ enabled: true, strength: p.zoomFollowStrength },
				p.videoAspect,
			);
		},
	};
}

/** Consume a job → encoded mp4 bytes. Closes the bitmaps it owns when done. */
export async function runExportJob(
	job: ExportJob,
	runtime: ExportRuntime = {},
): Promise<Uint8Array> {
	const overlayFactory = job.cursorSprites ? cursorOverlayFactory(job.cursorSprites) : null;
	const overlays = overlayFactory ? [overlayFactory] : [];
	const camera = job.camera ? reconstructCamera(job.camera) : null;

	const anno = job.annotation;
	// Missing/failed images are simply absent → getImage returns null → the paint
	// path draws its placeholder (same as the old `{ ready: false }` sentinel).
	const imageMap = anno
		? new Map<string, ShapeImage>(anno.images.map(([p, bmp]) => [p, { img: bmp, ready: true }]))
		: null;
	const annotationLayer =
		anno && imageMap
			? (ctx: OffscreenCanvasRenderingContext2D, t: number, blur: BlurLayerEnv) =>
					drawAnnotationLayerExport(ctx, t, {
						annotations: anno.annotations,
						meta: anno.meta,
						padding: anno.padding,
						outputAspect: anno.outputAspect,
						zoomRegions: anno.zoomRegions,
						canvasPxW: anno.canvasPxW,
						canvasPxH: anno.canvasPxH,
						getImage: (p) => imageMap.get(p) ?? null,
						blur,
					})
			: null;

	const cap = job.caption;
	const captionLayer = cap
		? (ctx: OffscreenCanvasRenderingContext2D, originalSec: number, outputSec: number) =>
				drawCaptionLayerExport(ctx, originalSec, outputSec, cap)
		: null;

	const frameAt = makeExportFrameAt(job.base, job.timeMap);
	try {
		return await renderTimelineToVideo({
			videoUrl: job.videoUrl,
			width: job.base.canvasPxW,
			height: job.base.canvasPxH,
			fps: job.fps,
			outputDurationSec: job.outputDurationSec,
			encodingConfig: job.encodingConfig,
			frameAt,
			backgroundImage: job.backgroundImage,
			overlays,
			camera,
			annotationLayer,
			captionLayer,
			onProgress: runtime.onProgress,
			signal: runtime.signal,
		});
	} finally {
		job.backgroundImage?.close();
		if (anno) for (const [, bmp] of anno.images) bmp.close();
	}
}
