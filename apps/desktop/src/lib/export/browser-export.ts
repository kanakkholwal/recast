/**
 * Browser export orchestrator (Phase 4c): snapshot the editor's scene, render the
 * whole timeline through the SAME RenderCore the preview uses, WebCodecs-encode it
 * to an mp4, persist it to a temp file, and return that path to hand to the Rust
 * mux job (`browserVideoPath`) — which copies the video (`-c:v copy`) and adds the
 * audio. One compositor, so preview and export can't diverge.
 *
 * MAIN-PASS scene only for now: background colour/gradient, video, zoom, shadow,
 * dot cursor, click highlight, scene entrance/exit anim. Overlays (sprite cursor,
 * camera bubble, captions, annotations, image backgrounds) are CON-7 follow-ups —
 * they render as DOM in preview today and get folded into the export RenderCore's
 * pass list next.
 */

import { saveBrowserExportVideo } from "$lib/ipc";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { buildGradientUniforms } from "../../components/editor/gradient.logic";
import { buildPressEvents } from "../../components/editor/cursor-animation.logic";
import { buildExportBase } from "./export-scene";
import { makeExportFrameAt } from "./export-frame-input";
import { videoEncodingConfigFor, type ExportQuality } from "./browser-export-plan";
import { renderTimelineToVideo } from "./offscreen-export";

export interface BrowserExportOptions {
	/** Source video asset URL (what the preview decodes, e.g. `convertFileSrc(...)`). */
	videoUrl: string;
	quality: ExportQuality;
	fps: number;
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
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
		backgroundImageReady: false,
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

	const frameAt = makeExportFrameAt(base, timeMap);
	const mp4 = await renderTimelineToVideo({
		videoUrl: opts.videoUrl,
		width: base.canvasPxW,
		height: base.canvasPxH,
		fps: opts.fps,
		outputDurationSec,
		encodingConfig: videoEncodingConfigFor(opts.quality),
		frameAt,
		onProgress: opts.onProgress,
		signal: opts.signal,
	});

	// Copy out of the (possibly larger) backing buffer so the transfer is exact.
	const bytes = mp4.buffer.slice(mp4.byteOffset, mp4.byteOffset + mp4.byteLength) as ArrayBuffer;
	return saveBrowserExportVideo(bytes);
}
