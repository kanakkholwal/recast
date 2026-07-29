/**
 * Shared caption renderer for BOTH the desktop preview overlay and the export
 * burn-in: resolve the active chunk at a source time, then paint it onto a 2D
 * context (preview canvas or export OffscreenCanvas) with CaptionBox's look. One
 * resolve + paint path means preview == export by construction — no third
 * renderer drifting from the DOM. Pure over @recast/captions + the cut-clip math;
 * no store, no DOM.
 */

import {
	activeChunkIndex,
	activeWordIndex,
	breakIntoLines,
	captionHeightFrac,
	captionTopFrac,
	chunkWords,
	isStaticAnimation,
	pillBox,
	resolveCaptionAnimation,
	spokenWordCount,
	withAlpha,
	wordColor,
	wordScaled,
	type CaptionAnimation,
	type CaptionStyle,
	type TranscriptWord,
} from "@recast/captions";
import {
	activeClippedSegment,
	captionSpanAt,
	keptCaptionSpans,
} from "$lib/captions/clip-with-cuts";
import { originalToOutput, type TimeMap } from "$lib/timeline/time-map";
import type { Transcript } from "$lib/ipc";

/** 2D surface shared by the preview canvas and the export offscreen canvas. */
export type CaptionCtx = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

/** The two caption clocks derived from the store playhead. `store.currentTime` is
 *  ORIGINAL (source) time, so it resolves the chunk directly; the entrance is
 *  clocked on OUTPUT time (viewer-rate) via the time map. Mirrors the export
 *  driver, which feeds `resolveCaptionView(originalSec)` + `paintCaptionChunk(outputSec)`. */
export function captionClocks(
	timeMap: TimeMap,
	currentTimeOriginal: number,
): { sourceSec: number; outputSec: number } {
	return {
		sourceSec: currentTimeOriginal,
		outputSec: originalToOutput(timeMap, currentTimeOriginal),
	};
}

/** The active caption at a source time: the chunk's words plus speech progress
 *  and entrance origin (in OUTPUT time, so the entrance plays at viewer-rate even
 *  across a speed change). Null when nothing is on screen. */
export interface CaptionView {
	words: TranscriptWord[];
	/** How many words are spoken (progressive highlight). */
	spoken: number;
	/** Currently-spoken word index, -1 if none. */
	activeIndex: number;
	/** OUTPUT-time start of the chunk, for the entrance clock. */
	chunkStartOutput: number;
	anim: CaptionAnimation;
}

/** Video rect inside the output frame + the render buffer size, all the painter
 *  needs to place a caption. */
export interface CaptionGeom {
	videoLeftFrac: number;
	videoRightFrac: number;
	videoTopFrac: number;
	videoBottomFrac: number;
	canvasPxW: number;
	canvasPxH: number;
}

/** Resolve the on-screen caption at SOURCE time `t`. Cut-clipped (breaks only at
 *  real cuts) and chunked. Pure — the caller gates visibility (preview `enabled`,
 *  export `burnIn`). */
export function resolveCaptionView(
	transcript: Transcript | null,
	style: CaptionStyle,
	timeMap: TimeMap,
	t: number,
): CaptionView | null {
	if (!transcript) return null;
	const span = captionSpanAt(keptCaptionSpans(timeMap), t);
	if (!span) return null;
	const clipped = activeClippedSegment(transcript.segments, span, t);
	if (!clipped) return null;
	// Reliability: show the FULL segment, not the words clipped to this span. The
	// span gate above already hides the caption during the cut gap (so it never
	// outlasts a cut), but a caption STRADDLING a cut keeps all its words on each
	// side instead of dropping the far-side ones and reading as "gone" — which
	// also stopped later captions from showing (owner: "just show the caption").
	const active = {
		start: clipped.segment.start,
		end: clipped.segment.end,
		text: clipped.segment.text,
		words: clipped.segment.words,
	};
	const anim = resolveCaptionAnimation(style.animation);
	const animated = active.words.length > 0 && !isStaticAnimation(anim);
	const outAt = (src: number) => originalToOutput(timeMap, src);

	if (active.words.length === 0) {
		const w = [{ start: active.start, end: active.end, text: active.text }];
		return { words: w, spoken: 1, activeIndex: -1, chunkStartOutput: outAt(active.start), anim };
	}
	if (!animated) {
		return {
			words: active.words,
			spoken: active.words.length,
			activeIndex: -1,
			chunkStartOutput: outAt(active.start),
			anim,
		};
	}
	const runs = chunkWords(active.words, anim);
	const ci = activeChunkIndex(runs, t);
	const chunk = runs[ci];
	if (!chunk) return null;
	return {
		words: chunk.words,
		spoken: spokenWordCount(chunk.words, t),
		activeIndex: activeWordIndex(chunk.words, t, anim.holdGaps),
		chunkStartOutput: outAt(chunk.words[0]?.start ?? active.start),
		anim,
	};
}

const clamp01 = (x: number) => Math.max(0, Math.min(1, x));
const easeOutCubic = (x: number) => 1 - Math.pow(1 - clamp01(x), 3);
const easeOutQuad = (x: number) => 1 - (1 - clamp01(x)) * (1 - clamp01(x));
const SCALE_EMPHASIS = 1.14;

/** Draw the resolved chunk with CaptionBox's look. `outputSec` is OUTPUT (viewer)
 *  time — the entrance clock — while the chunk's words were resolved at source
 *  time. Measurement is done here via the ctx; callers pass the placement. */
export function paintCaptionChunk(
	ctx: CaptionCtx,
	view: CaptionView,
	style: CaptionStyle,
	outputSec: number,
	geom: CaptionGeom,
): void {
	const { canvasPxW, canvasPxH } = geom;
	// cqh: the caption container fills the output canvas, so 1cqh = 1% of height.
	const fontPx = (style.fontSizePct / 100) * canvasPxH;
	if (!(fontPx > 0)) return;

	ctx.save();
	ctx.font = `${style.fontWeight} ${fontPx}px ${style.fontFamily}`;
	ctx.textBaseline = "middle";
	ctx.letterSpacing = `${style.letterSpacing * fontPx}px`;

	const cased = (s: string) => (style.uppercase ? s.toUpperCase() : s);
	const lines = breakIntoLines(view.words, style.maxCharsPerLine, style.maxLines);
	const spaceW = ctx.measureText(" ").width;
	const lineWidths = lines.map((line) =>
		line.reduce(
			(w, wi, k) => w + (k > 0 ? spaceW : 0) + ctx.measureText(cased(view.words[wi].text)).width,
			0,
		),
	);
	const widest = lineWidths.reduce((m, w) => Math.max(m, w), 0);
	const pill = pillBox(style, fontPx, widest, lines.length);

	// Horizontal band = the video rect, inset 4% (matching CaptionOverlay's
	// px-[4%]); the pill is justified within it.
	const bandLeft = geom.videoLeftFrac * canvasPxW;
	const bandWidth = (geom.videoRightFrac - geom.videoLeftFrac) * canvasPxW;
	const inset = 0.04 * bandWidth;
	const availLeft = bandLeft + inset;
	const availWidth = Math.max(0, bandWidth - 2 * inset);
	const pillX =
		style.align === "left"
			? availLeft
			: style.align === "right"
				? availLeft + availWidth - pill.width
				: availLeft + (availWidth - pill.width) / 2;

	// Vertical: captionTopFrac places the block's top edge, or centres on video.
	const cap = captionHeightFrac(style.fontSizePct, style.maxLines);
	const topFrac = captionTopFrac(style.position, style.offsetPct, cap, {
		top: geom.videoTopFrac,
		bottom: geom.videoBottomFrac,
	});
	const pillY =
		topFrac === null
			? ((geom.videoTopFrac + geom.videoBottomFrac) / 2) * canvasPxH - pill.height / 2
			: topFrac * canvasPxH;

	// Entrance: opacity + a scale/slide about the pill centre, on the OUTPUT-time
	// chunk clock (so it plays at viewer-rate even on a sped-up segment).
	const entranceSec = Math.max(0, view.anim.entranceMs) / 1000;
	let alpha = 1;
	let scale = 1;
	let dy = 0;
	if (view.anim.entrance !== "none" && entranceSec > 0) {
		const p = clamp01((outputSec - view.chunkStartOutput) / entranceSec);
		if (view.anim.entrance === "fade") {
			alpha = easeOutQuad(p);
		} else {
			const e = easeOutCubic(p);
			alpha = e;
			scale = 0.97 + 0.03 * e;
			if (view.anim.entrance === "slide") dy = 0.25 * fontPx * (1 - e);
		}
	}

	ctx.globalAlpha = alpha;
	if (scale !== 1 || dy !== 0) {
		const cx = pillX + pill.width / 2;
		const cy = pillY + pill.height / 2;
		ctx.translate(cx, cy + dy);
		ctx.scale(scale, scale);
		ctx.translate(-cx, -cy);
	}

	if (style.background === "box") {
		ctx.beginPath();
		roundRect(ctx, pillX, pillY, pill.width, pill.height, pill.radius);
		ctx.fillStyle = withAlpha(style.backgroundColor, style.backgroundOpacity / 100);
		ctx.fill();
	}

	const lineBox = style.lineHeight * fontPx;
	const contentLeft = pillX + pill.padX;
	const contentWidth = pill.width - 2 * pill.padX;
	const outlinePx = style.outlineWidth > 0 ? (style.outlineWidth / 100) * fontPx * 2 : 0;

	lines.forEach((line, li) => {
		const lineCenterY = pillY + pill.padY + (li + 0.5) * lineBox;
		let x =
			style.align === "left"
				? contentLeft
				: style.align === "right"
					? contentLeft + contentWidth - lineWidths[li]
					: contentLeft + (contentWidth - lineWidths[li]) / 2;
		line.forEach((wi, k) => {
			if (k > 0) x += spaceW;
			const text = cased(view.words[wi].text);
			const w = ctx.measureText(text).width;
			const color = wordColor({
				index: wi,
				activeIndex: view.activeIndex,
				spokenCount: view.spoken,
				wordCount: view.words.length,
				style,
				anim: view.anim,
			});
			const scaled = wordScaled({
				index: wi,
				activeIndex: view.activeIndex,
				wordCount: view.words.length,
				anim: view.anim,
			});
			drawWord(ctx, text, x, lineCenterY, w, color, scaled, style, outlinePx);
			x += w;
		});
	});

	ctx.restore();
}

function drawWord(
	ctx: CaptionCtx,
	text: string,
	x: number,
	y: number,
	w: number,
	color: string,
	scaled: boolean,
	style: CaptionStyle,
	outlinePx: number,
): void {
	ctx.save();
	if (scaled) {
		const cx = x + w / 2;
		ctx.translate(cx, y);
		ctx.scale(SCALE_EMPHASIS, SCALE_EMPHASIS);
		ctx.translate(-cx, -y);
	}
	if (style.background === "soft") {
		ctx.shadowColor = "rgba(0,0,0,0.85)";
		ctx.shadowBlur = 0.14 * (style.fontSizePct / 100) * ctx.canvas.height;
		ctx.shadowOffsetY = 0.04 * (style.fontSizePct / 100) * ctx.canvas.height;
	}
	// paint-order: stroke fill — the outline sits under the glyph fill.
	if (outlinePx > 0) {
		ctx.lineWidth = outlinePx;
		ctx.strokeStyle = style.outlineColor;
		ctx.lineJoin = "round";
		ctx.strokeText(text, x, y);
		ctx.shadowColor = "transparent";
		ctx.shadowBlur = 0;
	}
	ctx.fillStyle = color;
	ctx.fillText(text, x, y);
	ctx.restore();
}

function roundRect(ctx: CaptionCtx, x: number, y: number, w: number, h: number, r: number): void {
	const rr = Math.min(r, w / 2, h / 2);
	ctx.moveTo(x + rr, y);
	ctx.lineTo(x + w - rr, y);
	ctx.quadraticCurveTo(x + w, y, x + w, y + rr);
	ctx.lineTo(x + w, y + h - rr);
	ctx.quadraticCurveTo(x + w, y + h, x + w - rr, y + h);
	ctx.lineTo(x + rr, y + h);
	ctx.quadraticCurveTo(x, y + h, x, y + h - rr);
	ctx.lineTo(x, y + rr);
	ctx.quadraticCurveTo(x, y, x + rr, y);
	ctx.closePath();
}
