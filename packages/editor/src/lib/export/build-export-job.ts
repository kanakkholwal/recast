/**
 * Export job producer (main thread) — snapshots the editor scene and rasterises
 * every DOM-bound asset (background, cursor sprites, annotation images, caption
 * font) into a fully serializable {@link ExportJob} of plain data + transferable
 * bitmaps. This is the ONE place that touches the store or the DOM; the consumer
 * (run-export-job) is pure so it runs in the render worker.
 *
 * The scene itself is `store.toRenderState()`, the same value the preview hands
 * the engine, so there is nothing to keep in step between them.
 */

import type { MediaRef } from "@recast/media";
import { loadBackgroundBitmap } from "../../components/background-source";
import type { EditorStore } from "../../stores/editor-store.svelte";
import { computeCanvasGeometry } from "../canvas-geometry";
import { smoothCursorPath, smoothingStrengthToSigmaMs } from "../cursor/smoothing";
import { getEditorServices } from "../editor/services";
import { resolveEngineFont } from "../fonts/engine-font";
import { loadCursorSprites } from "../playback/cursor-sprites";
import { resolveCursorDataUrl } from "../registry";
import { toStatic } from "../state-snapshot.svelte";
import type { ExportQuality } from "./browser-export-plan";
import type { ExportJob } from "./export-job";
import { expandTextAnnotations } from "./rasterize-text";

export interface ExportJobInputs {
	/** Source video asset URL (`convertFileSrc(...)`). */
	videoUrl: MediaRef | string;
	/** Camera stream URL, or empty/undefined when none. */
	cameraUrl?: string;
	/** Milliseconds the camera track lags video frame 0, measured at capture. */
	cameraOffsetMs?: number;
	quality: ExportQuality;
	fps: number;
}

/**
 * The cursor track as the engine wants it: SMOOTHED, because the engine draws
 * what it is given and does no smoothing of its own.
 *
 * This is the same path the preview shows. The old export shipped the raw
 * samples, so its pointer took the recorded jitter that the preview had already
 * ironed out.
 */
function buildCursorTrack(store: EditorStore): unknown | null {
	const raw = store.cursorSamplesRaw ?? [];
	if (raw.length === 0) return null;
	const idlePeriods = store.cursorIdlePeriods ?? [];
	const cs = store.cursorSettings;
	const sigmaMs = smoothingStrengthToSigmaMs(cs.smoothing);
	if (sigmaMs <= 0) return toStatic({ samples: raw, idlePeriods });
	const smoothed = smoothCursorPath(raw, {
		sigmaMs,
		snapToClicks: cs.snapToClicks,
		snapWindowMs: cs.snapWindowMs,
	});
	return toStatic({ samples: smoothed.samples, idlePeriods });
}

/** Decode image annotations to transferable bitmaps (path → bitmap). Failed
 *  loads are omitted, and the engine skips an annotation with no asset. */
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
				// Rasterised-text annotations carry a data: URL; file refs go through the host's resolver.
				const src = p.startsWith("data:") ? p : getEditorServices().resolveAssetUrl(p);
				out.push([p, await createImageBitmap(await (await fetch(src)).blob())]);
			} catch {
				/* miss → omitted */
			}
		}),
	);
	return out;
}

/** Word timings for burned captions, or null when they are not burned in. Gated
 *  on the export `burnIn` intent plus a transcript, matching the Rust burn. */
function buildCaptionTrack(store: EditorStore): unknown | null {
	const transcript = store.captionTranscript;
	if (!store.captionExport.burnIn || !transcript || transcript.segments.length === 0) return null;
	return toStatic(transcript);
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

	// The composition's native size: no DPR cap, since export wants full res.
	const geom = computeCanvasGeometry(meta.width, meta.height, store.padding, store.outputAspect);
	const width = Math.max(1, Math.round(geom.canvasW));
	const height = Math.max(1, Math.round(geom.canvasH));

	const backgroundImage = await loadBackgroundBitmap(
		store.backgroundType,
		store.backgroundValue,
	).catch(() => null);

	const cs = store.cursorSettings;
	const cursorSprites =
		cs.enabled && cs.style !== "dot"
			? await loadCursorSprites(cs.style, resolveCursorDataUrl).catch(() => [])
			: [];

	// Neither the engine nor Rust has a font rasteriser, so text reaches the scene pre-rendered at composition resolution.
	const annotations = store.annotationsGloballyHidden
		? []
		: await expandTextAnnotations(store.annotationsByZ, width, height);
	const annotationImages = await preloadAnnotationBitmaps(annotations);

	const captionTrack = buildCaptionTrack(store);
	// Copied, not handed over: the resolver caches these bytes, and transferring would detach the cache for later exports.
	const font = captionTrack
		? await resolveEngineFont(store.captionStyle.fontFamily, store.captionStyle.fontWeight)
		: null;

	// De-proxy every store-sourced field so the job survives `postMessage`.
	return {
		// The rasterised annotations replace the authored ones; everything else is what the preview hands the engine.
		scene: { ...toStatic(store.toRenderState()), annotations: toStatic(annotations) },
		timeMap: toStatic(timeMap),
		outputDurationSec,
		fps: opts.fps,
		quality: opts.quality,
		videoUrl: opts.videoUrl,
		sourceWidth: meta.width,
		sourceHeight: meta.height,
		width,
		height,
		backgroundImage,
		cursorSprites,
		cursorTrack: buildCursorTrack(store),
		captionTrack,
		captionFont: font ? new Uint8Array(font.data) : null,
		annotationImages,
		camera:
			opts.cameraUrl && store.cameraOverlay.enabled
				? { url: opts.cameraUrl, offsetMs: opts.cameraOffsetMs ?? 0 }
				: null,
	};
}
