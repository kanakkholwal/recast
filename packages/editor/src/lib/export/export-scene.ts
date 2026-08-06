/**
 * Export scene snapshot (Phase 4c): assemble the static `FrameInput` base (every
 * field except `playbackTime`) from the editor's render state, so the offline
 * renderer composites each frame through the SAME RenderCore the preview uses.
 * Geometry is derived here from the source dims + padding + aspect; the render
 * buffer is the composition's native size (no DPR cap — export wants full res).
 *
 * Pure: no store, no DOM. The orchestrator gathers the store fields (and loads
 * the cursor track) and hands them in, keeping this testable.
 */

import { computeCanvasGeometry } from "../canvas-geometry";
import type { OutputAspect } from "../../stores/editor-store.svelte";
import type { FrameInput } from "../../components/frame-params";

/** Store-sourced scene fields — a FrameInput minus the values this module derives
 *  (geometry + render-buffer size) and the per-frame `playbackTime`. */
export type ExportSceneInputs = Omit<
	FrameInput,
	"playbackTime" | "geom" | "canvasPxW" | "canvasPxH"
> & {
	padding: number;
	outputAspect: OutputAspect;
};

/** Build the static export FrameInput base. `canvasPx*` = composition native size
 *  so the shader's `sx` scale is 1 and the export renders at full resolution. */
export function buildExportBase(i: ExportSceneInputs): Omit<FrameInput, "playbackTime"> {
	const geom = computeCanvasGeometry(i.meta.width, i.meta.height, i.padding, i.outputAspect);
	const { padding: _padding, outputAspect: _outputAspect, ...rest } = i;
	return {
		...rest,
		geom,
		canvasPxW: Math.max(1, Math.round(geom.canvasW)),
		canvasPxH: Math.max(1, Math.round(geom.canvasH)),
	};
}
