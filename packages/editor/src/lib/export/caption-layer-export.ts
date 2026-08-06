/**
 * Export wrapper over the shared caption renderer ({@link caption-render}): paints
 * burned captions onto the same comp-native 2D layer the annotations use, so the
 * browser export needs no Rust ASS burn. The resolve + paint path is shared with
 * the preview overlay, so preview == export by construction.
 */

import { paintCaptionChunk, resolveCaptionView } from "../captions/caption-render";
import type { Transcript } from "../wire-types";
import type { TimeMap } from "../timeline/time-map";
import type { CaptionStyle } from "@recast/captions";

/** The video rect inside the output frame, as fractions of the canvas (the
 *  caller resolves this from geometry — kept out so this module stays store-free
 *  and unit-testable). */
export interface CaptionVideoRect {
	leftFrac: number;
	rightFrac: number;
	topFrac: number;
	bottomFrac: number;
}

export interface CaptionLayerInputs {
	transcript: Transcript | null;
	style: CaptionStyle;
	timeMap: TimeMap;
	video: CaptionVideoRect;
	/** Comp-native render buffer size (px) — the layer canvas dimensions. */
	canvasPxW: number;
	canvasPxH: number;
}

/** Draw the caption layer for source time `t` (entrance clocked on `outputSec`),
 *  or nothing when no caption is active. Composited above annotations. */
export function drawCaptionLayerExport(
	ctx: OffscreenCanvasRenderingContext2D,
	t: number,
	outputSec: number,
	i: CaptionLayerInputs,
): void {
	const view = resolveCaptionView(i.transcript, i.style, i.timeMap, t);
	if (!view) return;
	paintCaptionChunk(ctx, view, i.style, outputSec, {
		videoLeftFrac: i.video.leftFrac,
		videoRightFrac: i.video.rightFrac,
		videoTopFrac: i.video.topFrac,
		videoBottomFrac: i.video.bottomFrac,
		canvasPxW: i.canvasPxW,
		canvasPxH: i.canvasPxH,
	});
}
