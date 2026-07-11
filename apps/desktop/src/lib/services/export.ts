/**
 * Export service: turns an editor project into a rendered file. Sits between
 * the UI and the Rust pipeline; the agent-facing surface a future MCP server
 * calls. Owns NO UI state; progress is surfaced via an optional `onState`
 * callback. See ./README.md for the headless-core layering.
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import { rasterizeCursorSprites } from "$lib/export/rasterize-cursor";
import { expandTextAnnotations } from "$lib/export/rasterize-text";
import {
	type ExportGifSettings,
	type ExportSpeed,
	type Transcript,
	enqueueExport as enqueueExportIpc,
} from "$lib/ipc";
import {
	type EditorRenderState,
	type EditorStore,
	type VideoMetadata,
	framePaddingPixels,
} from "$lib/stores/editor-store.svelte";
import { originalToOutput } from "$lib/timeline/time-map";

/** Optional progress hooks for the hybrid-raster "Preparing…" phase. Each fires
 *  as its lane starts/finishes so the UI can show sub-stage progress. Omit for
 *  headless callers that don't need staging feedback. */
export interface ExportPrepHooks {
	onText?(status: "running" | "done"): void;
	onCursor?(status: "running" | "done"): void;
	onSending?(status: "running" | "done"): void;
}

export interface BuildExportRenderStateOptions {
	hooks?: ExportPrepHooks;
}

export interface ExportRenderStatePayload {
	/** The render state to hand to {@link enqueueExport}. */
	renderState: EditorRenderState;
	metadata: VideoMetadata | null;
}

/**
 * Build the render payload the Rust pipeline consumes from a project: runs the
 * two hybrid-raster passes (text → PNG, cursor → sprite sheet) and honors the
 * per-lane enable toggles (focus/annotations/cuts) without mutating the store.
 * Fully serializable, so an agent can build, inspect, and pass it to {@link enqueueExport}.
 */
export async function buildExportRenderState(
	store: EditorStore,
	opts: BuildExportRenderStateOptions = {},
): Promise<ExportRenderStatePayload> {
	const { hooks } = opts;
	const renderState = store.toRenderState();
	const meta = store.metadata;
	const paddingPx = framePaddingPixels(renderState.padding ?? 0, meta);
	const canvasW = meta ? meta.width + paddingPx * 2 : 0;
	const canvasH = meta ? meta.height + paddingPx * 2 : 0;

	const hasText = renderState.annotations.some((a) => a.kind.kind === "text");
	const hasStyledCursor = store.cursorSettings.style !== "dot";
	hooks?.onText?.(hasText ? "running" : "done");
	hooks?.onCursor?.(hasStyledCursor ? "running" : "done");

	// Run both hybrid-raster passes in parallel: independent, and the cursor SVG
	// decode is non-trivial on cold boot (Image() onload is async even for blobs).
	const [expandedAnnotations, cursorSprites] = await Promise.all([
		expandTextAnnotations(renderState.annotations, canvasW, canvasH).then(
			(r) => {
				hooks?.onText?.("done");
				return r;
			},
		),
		rasterizeCursorSprites(
			store.cursorSettings.style,
			store.cursorSettings.size * 16,
		).then((r) => {
			hooks?.onCursor?.("done");
			return r;
		}),
	]);

	hooks?.onSending?.("running");
	// Hand the pipeline only the active set per lane toggle; store data is preserved.
	const finalRenderState: EditorRenderState = {
		...renderState,
		annotations: store.annotationsGloballyHidden ? [] : expandedAnnotations,
		zoomRegions: store.focusEnabled ? renderState.zoomRegions : [],
		// `effectiveCuts` = the flag-gated, lane-enabled subset, so the export
		// matches the previewed edit. Inactive cuts stay on the store, not here.
		cuts: store.effectiveCuts,
		cursorSpriteRest: cursorSprites?.rest,
		cursorSpritePress: cursorSprites?.press,
		cursorSpriteRightPress: cursorSprites?.rightPress,
		cursorSpriteDrag: cursorSprites?.drag,
		cursorSpriteHotspotRest: cursorSprites?.restHotspot,
		cursorSpriteHotspotPress: cursorSprites?.pressHotspot,
		cursorSpriteHotspotRightPress: cursorSprites?.rightPressHotspot,
		cursorSpriteHotspotDrag: cursorSprites?.dragHotspot,
		cursorSpriteSizePx: cursorSprites?.pixelSize,
	};
	hooks?.onSending?.("done");

	return { renderState: finalRenderState, metadata: meta };
}

function canLoadImage(src: string): Promise<boolean> {
	return new Promise((resolve) => {
		const img = new Image();
		img.onload = () => resolve(true);
		img.onerror = () => resolve(false);
		img.src = src;
	});
}

/**
 * File paths of image annotations whose source can't be loaded (missing, moved,
 * or undecodable), so the caller can warn before an export ships with them
 * silently absent. Image annotations store an absolute path, so moving the
 * project or deleting the file leaves a valid-looking editor but a broken export.
 * Skips data-URL images (rasterized text), hidden annotations, and the case
 * where all annotations are globally hidden.
 */
export async function findMissingImageAnnotations(store: EditorStore): Promise<string[]> {
	if (store.annotationsGloballyHidden) return [];
	const paths = new Set<string>();
	for (const a of store.annotations) {
		if (a.hidden) continue;
		if (a.kind.kind === "image" && a.kind.path && !a.kind.path.startsWith("data:")) {
			paths.add(a.kind.path);
		}
	}
	if (paths.size === 0) return [];
	const checks = await Promise.all(
		[...paths].map((p) => canLoadImage(convertFileSrc(p)).then((ok) => ({ p, ok }))),
	);
	return checks.filter((c) => !c.ok).map((c) => c.p);
}

/**
 * True when a VIDEO-anchored blur overlaps a visible zoom region in time. The
 * export composites a blur as a static FFmpeg crop→boxblur→overlay at a fixed
 * rectangle; FFmpeg can't move that rectangle per frame to follow the zoom's
 * per-frame scale/translate. So a video-anchored blur (which tracks the zoomed
 * content in the preview) stays put in export and can expose what it hid.
 * Frame-anchored blurs are static by design, so they're already correct and
 * don't trigger the warning; anchoring the blur to the frame is the fix.
 */
export function hasBlurUnderZoom(store: EditorStore): boolean {
	if (store.annotationsGloballyHidden || !store.focusEnabled) return false;
	const zooms = store.zoomRegions.filter((z) => !z.hidden);
	if (zooms.length === 0) return false;
	return store.annotations.some(
		(a) =>
			a.kind.kind === "blur" &&
			a.anchor !== "frame" &&
			!a.hidden &&
			zooms.some((z) => a.start < z.end && z.start < a.end),
	);
}

/** What to emit for generated captions on export. Built from the store via
 *  {@link buildCaptionExport}; `null`/empty when there's no transcript. */
export interface CaptionExportPayload {
	/** Burn captions into the video pixels. */
	burnCaptions: boolean;
	/** Subtitle sidecar to write next to the export (output-time), or null. */
	sidecar: { format: "vtt" | "srt"; transcript: Transcript } | null;
}

/** Map a transcript onto the OUTPUT timeline (trim + cuts + per-segment speed)
 *  so sidecar timings line up with the exported video, not the raw recording.
 *  Exported so ad-hoc sidecar exports (e.g. the Captions panel's SRT/VTT
 *  buttons) apply the same warp the export dialog and Cloud track do. */
export function toOutputTimeTranscript(store: EditorStore, src: Transcript): Transcript {
	const map = store.timeMap;
	const at = (t: number) => originalToOutput(map, t);
	const segments = src.segments
		.map((seg) => ({
			...seg,
			start: at(seg.start),
			end: at(seg.end),
			words: seg.words.map((w) => ({ ...w, start: at(w.start), end: at(w.end) })),
		}))
		// Drop segments that collapse to nothing (fully inside a removed range).
		.filter((seg) => seg.end - seg.start > 0.01);
	return { ...src, segments };
}

/**
 * Resolve the caption export plan from the store's transcript + export options.
 * Returns no-ops when no transcript has been generated, so callers can pass it
 * unconditionally ("only export captions when there are captions").
 */
export function buildCaptionExport(store: EditorStore): CaptionExportPayload {
	const transcript = store.transcript;
	const opts = store.captionExport;
	if (!transcript || transcript.segments.length === 0) {
		return { burnCaptions: false, sidecar: null };
	}
	return {
		burnCaptions: opts.burnIn && store.exportFormat !== "gif",
		sidecar:
			opts.sidecar === "none"
				? null
				: { format: opts.sidecar, transcript: toOutputTimeTranscript(store, transcript) },
	};
}

/**
 * Output-time transcript for Cloud's caption track, regenerated from the stored
 * transcript regardless of the export sidecar choice (Cloud always offers a
 * selectable track when captions exist). Null when there's no transcript.
 */
export function buildCloudCaptionTranscript(store: EditorStore): Transcript | null {
	const t = store.transcript;
	if (!t || t.segments.length === 0) return null;
	return toOutputTimeTranscript(store, t);
}

export interface RunExportOptions {
	/** Source media path (the recording file or project path). */
	inputPath: string;
	format: string;
	quality: string;
	/** Built via {@link buildExportRenderState}. */
	renderState: EditorRenderState;
	exportId: string;
	gifSettings?: ExportGifSettings;
	speed?: ExportSpeed;
	/** Output frame rate for MP4/WebM; `null`/omitted keeps source rate. */
	fps?: number | null;
	/** Caption emission (burn-in + sidecar). Built via {@link buildCaptionExport}. */
	captions?: CaptionExportPayload;
}

/**
 * Queue an export on the backend. The Rust export queue owns the run: it persists
 * the payload, executes it on the single serial worker (so two exports never
 * fight for CPU/GPU), writes the caption sidecar on success, and reports progress
 * via `export-state` events. Resolves once the job is durably queued; the export
 * then runs in the background and survives closing the editor that built it.
 *
 * Editor-independent, so a headless/MCP caller can build a render state and hand
 * it off the same way. Progress + completion are observed via the queue (see
 * `listExportJobs` / `export-state`), not a returned promise of the output path.
 */
export async function enqueueExport(opts: RunExportOptions): Promise<void> {
	await enqueueExportIpc({
		inputPath: opts.inputPath,
		format: opts.format,
		quality: opts.quality,
		renderState: opts.renderState,
		exportId: opts.exportId,
		gifSettings: opts.gifSettings,
		speed: opts.speed,
		fps: opts.fps,
		burnCaptions: opts.captions?.burnCaptions ?? false,
		captionSidecar: opts.captions?.sidecar ?? null,
	});
}
