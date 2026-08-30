/**
 * Export service: turns an editor project into a rendered file. Sits between
 * the UI and the Rust pipeline; the agent-facing surface a future MCP server
 * calls. Owns NO UI state; progress is surfaced via an optional `onState`
 * callback. See ./README.md for the headless-core layering.
 */

import { getEditorServices } from "../editor/services";

// The native render queue; absent where the browser compositor is the only engine, which surfaces the failure.
function enqueueViaSink(job: unknown): Promise<string[]> {
	const enqueue = getEditorServices().exportSink?.enqueue;
	if (!enqueue) throw new Error("this host has no native export queue");
	return enqueue(job);
}

import {
	type EditorRenderState,
	type EditorStore,
	framePaddingPixels,
	type VideoMetadata,
} from "../../stores/editor-store.svelte";
import { toOutputTimeTranscript } from "../captions/output-time";
import { rasterizeCursorSprites } from "../export/rasterize-cursor";
import { expandTextAnnotations } from "../export/rasterize-text";
import type { ExportGifSettings, ExportSpeed, Transcript } from "../wire-types";

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
	/** Skip the text→PNG / cursor-sprite rasterization. The browser engine
	 *  composites those itself, and the mux job never reads them — so doing it here
	 *  too is pure double work. Audio/cuts/speed/metadata are unaffected. */
	skipVisualRaster?: boolean;
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
	const { hooks, skipVisualRaster } = opts;
	const renderState = store.toRenderState();
	const meta = store.metadata;

	// The browser engine rasterizes visuals itself, so skip those passes; annotations are dropped because Rust only knows image kinds.
	if (skipVisualRaster) {
		return {
			renderState: {
				...renderState,
				annotations: [],
				zoomRegions: store.focusEnabled ? renderState.zoomRegions : [],
				cuts: store.effectiveCuts,
			},
			metadata: meta,
		};
	}

	const paddingPx = framePaddingPixels(renderState.padding ?? 0, meta);
	const canvasW = meta ? meta.width + paddingPx * 2 : 0;
	const canvasH = meta ? meta.height + paddingPx * 2 : 0;

	const hasText = renderState.annotations.some((a) => a.kind.kind === "text");
	const hasStyledCursor = store.cursorSettings.style !== "dot";
	hooks?.onText?.(hasText ? "running" : "done");
	hooks?.onCursor?.(hasStyledCursor ? "running" : "done");

	// Independent, and the cursor SVG decode is non-trivial cold, since Image() onload is async even for blobs.
	const [expandedAnnotations, cursorSprites] = await Promise.all([
		expandTextAnnotations(renderState.annotations, canvasW, canvasH).then((r) => {
			hooks?.onText?.("done");
			return r;
		}),
		rasterizeCursorSprites(store.cursorSettings.style, store.cursorSettings.size * 16).then((r) => {
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
		// `effectiveCuts` is the flag-gated, lane-enabled subset, so the export matches the previewed edit.
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
	const resolve = getEditorServices().resolveAssetUrl;
	const checks = await Promise.all(
		[...paths].map((p) => canLoadImage(resolve(p)).then((ok) => ({ p, ok }))),
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

/** Re-exported so existing callers (export dialog, Cloud track, Captions
 *  panel) keep one import site; the math lives with the other caption logic. */
export { toOutputTimeTranscript };

/**
 * Resolve the caption export plan from the store's transcript + export options.
 * Returns no-ops when no transcript has been generated, so callers can pass it
 * unconditionally ("only export captions when there are captions").
 */
export function buildCaptionExport(store: EditorStore): CaptionExportPayload {
	// Rescaled onto the video and timeMap axis so sidecar cue times line up with the exported frames.
	const transcript = store.captionTranscript;
	const opts = store.captionExport;
	if (!transcript || transcript.segments.length === 0) {
		return { burnCaptions: false, sidecar: null };
	}
	return {
		burnCaptions: opts.burnIn && store.exportFormat !== "gif",
		sidecar:
			opts.sidecar === "none"
				? null
				: { format: opts.sidecar, transcript: toOutputTimeTranscript(store.timeMap, transcript) },
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
	return toOutputTimeTranscript(store.timeMap, t);
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
	/** Browser-rendered composited video temp path (Phase 4). When set, the job
	 *  mux-copies it instead of running the Rust filter_complex compositor. */
	browserVideoPath?: string;
	/** The editor's resolved kept-timeline, from {@link exportTimeMap}. Sending
	 *  it makes the backend REPLAY the editor's axis instead of re-deriving it
	 *  from cuts + splits + speed anchors, which is what used to let the two
	 *  disagree. Omit only from headless callers with no editor session. */
	timeMap?: ExportTimeSpan[] | null;
}

/** One kept span of the timeline in original-recording seconds. Mirrors
 *  `cuts_speed::TimeSpanWire` on the Rust side. */
export interface ExportTimeSpan {
	origStart: number;
	origEnd: number;
	speed: number;
}

/** The store's time map as the export wire format. */
export function exportTimeMap(map: {
	spans: ReadonlyArray<{ origStart: number; origEnd: number; speed: number }>;
}): ExportTimeSpan[] {
	return map.spans.map((s) => ({
		origStart: s.origStart,
		origEnd: s.origEnd,
		speed: s.speed,
	}));
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
export async function enqueueExport(opts: RunExportOptions): Promise<string[]> {
	// Returns any auto-repairs the backend applied, so a caller can surface a verify-this notice; empty means none.
	return enqueueViaSink({
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
		browserVideoPath: opts.browserVideoPath ?? null,
		timeMap: opts.timeMap ?? null,
	});
}
