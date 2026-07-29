/**
 * Browser export orchestrator (Phase 4c): snapshot the editor's scene, render the
 * whole timeline through the SAME RenderCore the preview uses, WebCodecs-encode it
 * to an mp4, persist it to a temp file, and return that path to hand to the Rust
 * mux job (`browserVideoPath`) — which copies the video (`-c:v copy`) and adds the
 * audio. One compositor, so preview and export can't diverge.
 *
 * Scene rendered today: background colour/gradient/image, video, zoom, shadow,
 * dot + sprite cursor, click highlight, scene entrance/exit anim, camera bubble,
 * and every annotation kind (painted/text/blur). GIF renders here too — the Rust
 * side then runs only the palette pass on this video. Only burned captions still
 * route to the Rust compositor (see {@link browserExportBlockedReason}).
 */

import { saveBrowserExportVideo } from "$lib/ipc";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { buildGradientUniforms } from "../../components/editor/gradient.logic";
import { computeCanvasGeometry } from "$lib/canvas-geometry";
import { loadBackgroundBitmap } from "../../components/editor/background-source";
import { buildPressEvents } from "../../components/editor/cursor-animation.logic";
import {
	applyZoomFollow,
	cameraFollowScaleAt,
	cameraPlacementAt,
} from "../../components/editor/_components/camera-overlay.logic";
import type { CameraExportInputs } from "./offscreen-export";
import type { FrameGeometry } from "../../components/editor/frame-params";
import { buildExportBase } from "./export-scene";
import { makeExportFrameAt } from "./export-frame-input";
import { videoEncodingConfigFor, type ExportQuality } from "./browser-export-plan";
import {
	renderTimelineToVideo,
	type BlurLayerEnv,
	type ExportOverlayFactory,
} from "./offscreen-export";
import { rasterizeCursorSprites } from "./rasterize-cursor";
import { cursorOverlayFactory } from "./cursor-overlay-export";
import { drawAnnotationLayerExport } from "./annotation-layer-export";
import { drawCaptionLayerExport } from "./caption-layer-export";
import { expandTextAnnotations } from "./rasterize-text";
import { ensureFontLoaded } from "$lib/fonts/font-options";
import type { ShapeImage } from "@recast/render";
import { convertFileSrc } from "@tauri-apps/api/core";

export interface BrowserExportOptions {
	/** Source video asset URL (what the preview decodes, e.g. `convertFileSrc(...)`). */
	videoUrl: string;
	/** Camera stream URL (`convertFileSrc(camera.mp4)`), or empty when none. */
	cameraUrl?: string;
	quality: ExportQuality;
	fps: number;
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** Camera bubble inputs from the store, or null when no camera / disabled. The
 *  effective placement mirrors CameraOverlay: base keyframe glide, then the
 *  zoom-follow grow/drift (gated on focus), using the shared pure helpers. */
function buildCameraInputs(
	store: EditorStore,
	cameraUrl: string | undefined,
	geom: FrameGeometry,
): CameraExportInputs | null {
	const cam = store.cameraOverlay;
	if (!cameraUrl || !cam.enabled) return null;
	const videoAspect = geom.videoH > 0 ? geom.videoW / geom.videoH : 1;
	const placementAt = (t: number) => {
		const b = cameraPlacementAt(cam.defaultPlacement, cam.keyframes, t, cam.keyframeEasing);
		if (!cam.zoomFollow || !store.focusEnabled) return b;
		const zoom = cameraFollowScaleAt(
			store.zoomRegions,
			t,
			cam.zoomFollowDuration,
			cam.zoomFollowEasing,
		);
		return applyZoomFollow(
			b,
			zoom,
			{ enabled: true, strength: cam.zoomFollowStrength },
			videoAspect,
		);
	};
	return {
		url: cameraUrl,
		geom,
		shape: cam.shape,
		cornerRadius: cam.cornerRadius,
		mirror: cam.mirror,
		placementAt,
	};
}

/** Rasterize the SVG cursor sprites (bitmaps + hotspots) into an export overlay
 *  factory, or null for the dot style / disabled cursor (the main shader draws
 *  those). Bitmaps are consumed by the factory on upload. */
async function buildCursorOverlay(store: EditorStore): Promise<ExportOverlayFactory | null> {
	const cs = store.cursorSettings;
	if (!cs.enabled || cs.style === "dot") return null;
	const bundle = await rasterizeCursorSprites(cs.style, cs.size * 16).catch(() => null);
	if (!bundle) return null;
	const toBmp = async (u: string) => createImageBitmap(await (await fetch(u)).blob());
	const rest = await toBmp(bundle.rest);
	const press = bundle.press === bundle.rest ? rest : await toBmp(bundle.press);
	return cursorOverlayFactory({
		rest,
		press,
		rightPress: bundle.rightPress ? await toBmp(bundle.rightPress) : undefined,
		drag: bundle.drag ? await toBmp(bundle.drag) : undefined,
		restHotspot: bundle.restHotspot,
		pressHotspot: bundle.pressHotspot,
		rightPressHotspot: bundle.rightPressHotspot,
		dragHotspot: bundle.dragHotspot,
	});
}

/** Decode every image annotation up front (path → drawable), so the per-frame
 *  layer draw is synchronous. Failed loads render as the placeholder box. */
async function preloadAnnotationImages(
	annotations: ReadonlyArray<{ hidden?: boolean; kind: { kind: string; path?: string } }>,
): Promise<Map<string, ShapeImage>> {
	const paths = new Set<string>();
	for (const a of annotations) {
		if (!a.hidden && a.kind.kind === "image" && a.kind.path) paths.add(a.kind.path);
	}
	const map = new Map<string, ShapeImage>();
	await Promise.all(
		[...paths].map(async (p) => {
			try {
				const img = new Image();
				img.crossOrigin = "anonymous";
				// Rasterized-text annotations carry a data: URL; file paths go through
				// the asset protocol.
				img.src = p.startsWith("data:") ? p : convertFileSrc(p);
				await img.decode();
				map.set(p, { img, ready: true });
			} catch {
				map.set(p, { img: new Image(), ready: false });
			}
		}),
	);
	return map;
}

/** Build the per-frame annotation-layer draw callback, or null when there's
 *  nothing to draw. Text annotations are rasterized to comp-resolution images up
 *  front (parity with the Rust path + preview) so the image path handles them;
 *  blur still routes to Rust upstream. */
async function buildAnnotationLayer(
	store: EditorStore,
	meta: { width: number; height: number },
	canvasPxW: number,
	canvasPxH: number,
): Promise<
	((ctx: OffscreenCanvasRenderingContext2D, t: number, blur: BlurLayerEnv) => void) | null
> {
	if (store.annotationsGloballyHidden) return null;
	const annotations = await expandTextAnnotations(store.annotationsByZ, canvasPxW, canvasPxH);
	const drawable = ["rect", "ellipse", "arrow", "image", "blur"];
	if (!annotations.some((a) => !a.hidden && drawable.includes(a.kind.kind))) return null;
	const images = await preloadAnnotationImages(annotations);
	return (ctx, t, blur) =>
		drawAnnotationLayerExport(ctx, t, {
			annotations,
			meta,
			padding: store.padding,
			outputAspect: store.outputAspect,
			zoomRegions: store.zoomRegions,
			canvasPxW,
			canvasPxH,
			getImage: (p) => images.get(p) ?? null,
			blur,
		});
}

/** Build the per-frame caption burn-in callback, or null when captions aren't
 *  burned in (no transcript, burn-in off, or GIF). The font is preloaded so the
 *  first frame measures/draws with the right face, mirroring the preview. */
async function buildCaptionLayer(
	store: EditorStore,
	meta: { width: number; height: number },
	canvasPxW: number,
	canvasPxH: number,
): Promise<((ctx: OffscreenCanvasRenderingContext2D, t: number) => void) | null> {
	const transcript = store.transcript;
	const style = store.captionStyle;
	const burn = store.captionExport.burnIn && store.exportFormat !== "gif";
	if (!burn || !transcript || transcript.segments.length === 0 || !style.enabled) return null;
	// Kick off the face load, then wait for all pending fonts so the first burned
	// frame measures/draws with the real face (the export can't repaint later).
	ensureFontLoaded(style.fontFamily, style.fontWeight);
	try {
		await document.fonts.ready;
	} catch {
		/* fall back to the system face */
	}
	const g = computeCanvasGeometry(meta.width, meta.height, store.padding, store.outputAspect);
	const video = {
		leftFrac: g.videoX / g.canvasW,
		rightFrac: (g.videoX + g.videoW) / g.canvasW,
		topFrac: g.videoY / g.canvasH,
		bottomFrac: (g.videoY + g.videoH) / g.canvasH,
	};
	return (ctx, t) =>
		drawCaptionLayerExport(ctx, t, {
			transcript,
			style,
			timeMap: store.timeMap,
			video,
			canvasPxW,
			canvasPxH,
		});
}

/** Render + encode the timeline in the browser; resolves with the temp path of
 *  the video-only mp4 to mux server-side. Throws if the source isn't ready. */
export async function runBrowserExport(
	store: EditorStore,
	opts: BrowserExportOptions,
): Promise<string> {
	const meta = store.metadata;
	if (!meta?.width || !meta?.height) throw new Error("browser export: source metadata not ready");
	const timeMap = store.timeMap;
	const outputDurationSec = timeMap.outputDuration;
	if (!(outputDurationSec > 0)) throw new Error("browser export: empty output timeline");

	const gradient =
		store.backgroundType === "gradient"
			? buildGradientUniforms(store.backgroundValue || "")
			: undefined;
	// Image/wallpaper backgrounds decode once to a texture; null keeps the flat
	// fallback (the main pass renders colour/gradient without one).
	const backgroundImage = await loadBackgroundBitmap(
		store.backgroundType,
		store.backgroundValue,
	).catch(() => null);
	// Cursor comes from the store's published raw samples (the preview loaded them);
	// press events derive from those. Idle-hide + smoothing are follow-ups.
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
		idlePeriods: [],
		pressEvents: buildPressEvents(cursorSamples),
	});

	const cursorOverlay = await buildCursorOverlay(store);
	const overlays = cursorOverlay ? [cursorOverlay] : [];
	const camera = buildCameraInputs(store, opts.cameraUrl, base.geom);
	const annotationLayer = await buildAnnotationLayer(
		store,
		{ width: meta.width, height: meta.height },
		base.canvasPxW,
		base.canvasPxH,
	);
	const captionLayer = await buildCaptionLayer(
		store,
		{ width: meta.width, height: meta.height },
		base.canvasPxW,
		base.canvasPxH,
	);

	const frameAt = makeExportFrameAt(base, timeMap);
	try {
		const mp4 = await renderTimelineToVideo({
			videoUrl: opts.videoUrl,
			width: base.canvasPxW,
			height: base.canvasPxH,
			fps: opts.fps,
			outputDurationSec,
			encodingConfig: videoEncodingConfigFor(opts.quality),
			frameAt,
			backgroundImage,
			overlays,
			camera,
			annotationLayer,
			captionLayer,
			onProgress: opts.onProgress,
			signal: opts.signal,
		});

		// Copy out of the (possibly larger) backing buffer so the transfer is exact.
		const bytes = mp4.buffer.slice(mp4.byteOffset, mp4.byteOffset + mp4.byteLength) as ArrayBuffer;
		return await saveBrowserExportVideo(bytes);
	} finally {
		backgroundImage?.close();
	}
}
