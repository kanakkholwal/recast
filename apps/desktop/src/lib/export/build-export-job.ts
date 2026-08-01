/**
 * Export job producer (main thread) — snapshots the editor scene and rasterizes
 * every DOM-bound asset (background, cursor sprites, annotation images, caption
 * font) into a fully serializable {@link ExportJob} of plain data + transferable
 * bitmaps. This is the ONE place that touches the store / DOM; the consumer
 * (run-export-job) is pure so it can move into the render worker (Phase 3).
 */

import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { buildGradientUniforms } from "../../components/editor/gradient.logic";
import { computeCanvasGeometry } from "$lib/canvas-geometry";
import { loadBackgroundBitmap } from "../../components/editor/background-source";
import { buildPressEvents } from "../../components/editor/cursor-animation.logic";
import { buildExportBase } from "./export-scene";
import type { ExportQuality } from "./browser-export-plan";
import { rasterizeCursorSprites } from "./rasterize-cursor";
import { expandTextAnnotations } from "./rasterize-text";
import { planCaptionFont } from "$lib/fonts/font-options";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toStatic } from "$lib/state-snapshot.svelte";
import type { FrameGeometry } from "../../components/editor/frame-params";
import type { CursorSpriteSources } from "./cursor-overlay-export";
import type { ExportJob, CameraJob, AnnotationJob, CaptionJob } from "./export-job";

export interface ExportJobInputs {
	/** Source video asset URL (`convertFileSrc(...)`). */
	videoUrl: string;
	/** Camera stream URL, or empty/undefined when none. */
	cameraUrl?: string;
	quality: ExportQuality;
	fps: number;
}

/** Rasterize the SVG cursor sprites to transferable bitmaps + hotspots, or null
 *  for the dot style / disabled cursor (the main shader draws those). */
async function buildCursorSprites(store: EditorStore): Promise<CursorSpriteSources | null> {
	const cs = store.cursorSettings;
	if (!cs.enabled || cs.style === "dot") return null;
	const bundle = await rasterizeCursorSprites(cs.style, cs.size * 16).catch(() => null);
	if (!bundle) return null;
	const toBmp = async (u: string) => createImageBitmap(await (await fetch(u)).blob());
	const rest = await toBmp(bundle.rest);
	// Share one bitmap when a state reuses the rest sprite (the consumer dedupes on ===).
	const press = bundle.press === bundle.rest ? rest : await toBmp(bundle.press);
	return {
		rest,
		press,
		rightPress: bundle.rightPress ? await toBmp(bundle.rightPress) : undefined,
		drag: bundle.drag ? await toBmp(bundle.drag) : undefined,
		restHotspot: bundle.restHotspot,
		pressHotspot: bundle.pressHotspot,
		rightPressHotspot: bundle.rightPressHotspot,
		dragHotspot: bundle.dragHotspot,
	};
}

/** Camera bubble as data (placementAt is rebuilt by the consumer), or null when
 *  there's no camera / it's disabled. Mirrors the old buildCameraInputs. */
function buildCameraData(
	store: EditorStore,
	cameraUrl: string | undefined,
	geom: FrameGeometry,
): CameraJob | null {
	const cam = store.cameraOverlay;
	if (!cameraUrl || !cam.enabled) return null;
	const videoAspect = geom.videoH > 0 ? geom.videoW / geom.videoH : 1;
	return {
		url: cameraUrl,
		geom,
		shape: cam.shape,
		cornerRadius: cam.cornerRadius,
		mirror: cam.mirror,
		placement: {
			defaultPlacement: cam.defaultPlacement,
			keyframes: cam.keyframes,
			keyframeEasing: cam.keyframeEasing,
			zoomFollow: cam.zoomFollow,
			focusEnabled: store.focusEnabled,
			zoomRegions: store.zoomRegions,
			zoomFollowDuration: cam.zoomFollowDuration,
			zoomFollowEasing: cam.zoomFollowEasing,
			zoomFollowStrength: cam.zoomFollowStrength,
			videoAspect,
		},
	};
}

/** Decode image annotations to transferable bitmaps (path → bitmap). Failed loads
 *  are omitted; the consumer's getImage returns null → placeholder box. */
async function preloadAnnotationBitmaps(
	annotations: ReadonlyArray<{ hidden?: boolean; kind: { kind: string; path?: string } }>,
): Promise<Array<[string, ImageBitmap]>> {
	const paths = new Set<string>();
	for (const a of annotations) {
		if (!a.hidden && a.kind.kind === "image" && a.kind.path) paths.add(a.kind.path);
	}
	const out: Array<[string, ImageBitmap]> = [];
	await Promise.all(
		[...paths].map(async (p) => {
			try {
				// Rasterized-text annotations carry a data: URL; file paths go through
				// the asset protocol.
				const src = p.startsWith("data:") ? p : convertFileSrc(p);
				out.push([p, await createImageBitmap(await (await fetch(src)).blob())]);
			} catch {
				/* miss → omitted; consumer draws the placeholder */
			}
		}),
	);
	return out;
}

/** Annotation layer as data, or null when nothing draws. Text annotations are
 *  rasterized to comp-resolution images up front (parity with preview + Rust). */
async function buildAnnotationData(
	store: EditorStore,
	meta: { width: number; height: number },
	canvasPxW: number,
	canvasPxH: number,
): Promise<AnnotationJob | null> {
	if (store.annotationsGloballyHidden) return null;
	const annotations = await expandTextAnnotations(store.annotationsByZ, canvasPxW, canvasPxH);
	const drawable = ["rect", "ellipse", "arrow", "image", "blur"];
	if (!annotations.some((a) => !a.hidden && drawable.includes(a.kind.kind))) return null;
	const images = await preloadAnnotationBitmaps(annotations);
	return {
		annotations,
		meta,
		padding: store.padding,
		outputAspect: store.outputAspect,
		zoomRegions: store.zoomRegions,
		canvasPxW,
		canvasPxH,
		images,
	};
}

/** Caption layer as data, or null when captions aren't burned in. Gated on the
 *  export `burnIn` intent + a transcript ONLY (matching the Rust burn). The font
 *  URL is resolved here (Tauri IPC) so the consumer — worker OR main — registers
 *  it in its own scope before the first frame; the export can't repaint. */
async function buildCaptionData(
	store: EditorStore,
	meta: { width: number; height: number },
	canvasPxW: number,
	canvasPxH: number,
): Promise<CaptionJob | null> {
	const transcript = store.captionTranscript;
	const style = store.captionStyle;
	if (!store.captionExport.burnIn || !transcript || transcript.segments.length === 0) return null;
	const plan = await planCaptionFont(style.fontFamily, style.fontWeight);
	const g = computeCanvasGeometry(meta.width, meta.height, store.padding, store.outputAspect);
	return {
		transcript,
		style,
		timeMap: store.timeMap,
		video: {
			leftFrac: g.videoX / g.canvasW,
			rightFrac: (g.videoX + g.videoW) / g.canvasW,
			topFrac: g.videoY / g.canvasH,
			bottomFrac: (g.videoY + g.videoH) / g.canvasH,
		},
		canvasPxW,
		canvasPxH,
		font: plan.where === "worker" ? plan.font : undefined,
		mainThreadOnly: plan.where === "main",
	};
}

/** Snapshot the editor scene + assets into a serializable job. Throws if the
 *  source isn't ready or the output timeline is empty. */
export async function buildExportJob(
	store: EditorStore,
	opts: ExportJobInputs,
): Promise<ExportJob> {
	const meta = store.metadata;
	if (!meta?.width || !meta?.height) throw new Error("browser export: source metadata not ready");
	const timeMap = store.timeMap;
	const outputDurationSec = timeMap.outputDuration;
	if (!(outputDurationSec > 0)) throw new Error("browser export: empty output timeline");

	const gradient =
		store.backgroundType === "gradient"
			? buildGradientUniforms(store.backgroundValue || "")
			: undefined;
	const backgroundImage = await loadBackgroundBitmap(
		store.backgroundType,
		store.backgroundValue,
	).catch(() => null);
	// Cursor comes from the store's published raw samples (the preview loaded them);
	// press events + idle spans derive from the same track.
	const cursorSamples = store.cursorSamplesRaw ?? [];
	const base = buildExportBase({
		meta: { width: meta.width, height: meta.height },
		padding: store.padding,
		outputAspect: store.outputAspect,
		segments: store.segments,
		segmentAnims: store.segmentAnims,
		backgroundType: store.backgroundType,
		backgroundValue: store.backgroundValue,
		backgroundBlur: store.backgroundBlur,
		backgroundImageReady: backgroundImage != null,
		gradient,
		borderRadius: store.borderRadius ?? 0,
		focusEnabled: store.focusEnabled,
		zoomRegions: store.zoomRegions,
		shadow: store.shadow,
		cursor: store.cursorSettings,
		cursorMotionEasing: store.cursorMotionEasing,
		cursorSamples,
		idlePeriods: store.cursorIdlePeriods ?? [],
		pressEvents: buildPressEvents(cursorSamples),
	});
	const metaWH = { width: meta.width, height: meta.height };
	const camera = buildCameraData(store, opts.cameraUrl, base.geom);
	const annotation = await buildAnnotationData(store, metaWH, base.canvasPxW, base.canvasPxH);
	const caption = await buildCaptionData(store, metaWH, base.canvasPxW, base.canvasPxH);

	// De-proxy every store-sourced field so the job survives `postMessage` to the
	// render worker; bitmaps (built locally) stay out of the snapshot.
	return {
		base: toStatic(base),
		timeMap: toStatic(timeMap),
		outputDurationSec,
		fps: opts.fps,
		quality: opts.quality,
		videoUrl: opts.videoUrl,
		backgroundImage,
		cursorSprites: await buildCursorSprites(store),
		camera: camera ? toStatic(camera) : null,
		annotation: staticAnnotation(annotation),
		caption: caption ? toStatic(caption) : null,
	};
}

/** Snapshot the annotation data while preserving its transferable bitmaps. */
function staticAnnotation(a: AnnotationJob | null): AnnotationJob | null {
	if (!a) return null;
	const { images, ...rest } = a;
	return { ...toStatic(rest), images };
}
