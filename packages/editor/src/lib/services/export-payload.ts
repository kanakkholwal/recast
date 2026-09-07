/**
 * The export wire payload and the types it is built from. Split from
 * `export.ts` because that module reaches into runes state, and the field list
 * crossing to Rust is the one thing that has to be testable on its own: a
 * renamed or dropped key fails nothing at either compiler.
 */

import type { EditorRenderState } from "../../stores/editor-store.svelte";
import type { ExportGifSettings, ExportSpeed, Transcript } from "../wire-types";

/** What the export should emit for captions. */
export interface CaptionExportPayload {
	/** Burn captions into the video pixels. */
	burnCaptions: boolean;
	/** Subtitle sidecar to write next to the export (output-time), or null. */
	sidecar: { format: "vtt" | "srt"; transcript: Transcript } | null;
}

export interface RunExportOptions {
	/** Source media path (the recording file or project path). */
	inputPath: string;
	format: string;
	quality: string;
	/** Built via `buildExportRenderState`. */
	renderState: EditorRenderState;
	exportId: string;
	gifSettings?: ExportGifSettings;
	speed?: ExportSpeed;
	/** Output frame rate for MP4/WebM; `null`/omitted keeps source rate. */
	fps?: number | null;
	/** Caption emission (burn-in + sidecar). Built via `buildCaptionExport`. */
	captions?: CaptionExportPayload;
	/** Browser-rendered composited video temp path (Phase 4). When set, the job
	 *  mux-copies it instead of running the Rust filter_complex compositor. */
	browserVideoPath?: string;
	/** The editor's resolved kept-timeline, from `exportTimeMap`. Sending it
	 *  makes the backend REPLAY the editor's axis instead of re-deriving it from
	 *  cuts + splits + speed anchors, which is what used to let the two
	 *  disagree. Omit only from headless callers with no editor session. */
	timeMap?: ExportTimeSpan[] | null;
	/** Render through the engine instead of the FFmpeg filtergraph. From the
	 *  `engineExport` experimental flag; `RECAST_ENGINE_EXPORT` overrides it. */
	engineExport?: boolean;
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
 * The payload the backend's `ExportRequest` deserializes, field for field.
 *
 * Optionals are sent as explicit `null` rather than omitted, so a headless
 * caller and the editor produce the same shape and serde's defaults never have
 * to stand in for a decision the caller made.
 */
export function exportPayload(opts: RunExportOptions) {
	return {
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
		engineExport: opts.engineExport ?? false,
	};
}
