/**
 * Pill geometry from style plus a measured text width. Pure arithmetic, no
 * measurement of its own: the caller supplies `textWidthPx` (the DOM measures
 * with the browser, Rust measures with rustybuzz), and both then derive the
 * same pill box off it. Mirrored in Rust.
 */

import type { CaptionStyle } from "./types";

export interface PillBox {
	/** Pill width in px: widest line + horizontal padding both sides. */
	width: number;
	/** Pill height in px: line count * line height + vertical padding both sides. */
	height: number;
	/** Corner radius in px, clamped to half the pill height. */
	radius: number;
	/** Horizontal padding in px (one side). */
	padX: number;
	/** Vertical padding in px (one side). */
	padY: number;
}

/**
 * @param fontPx      resolved font size in px (CSS em box).
 * @param textWidthPx width of the widest rendered line, in px.
 * @param lineCount   number of wrapped lines.
 */
export function pillBox(
	style: CaptionStyle,
	fontPx: number,
	textWidthPx: number,
	lineCount: number,
): PillBox {
	const padX = style.boxPaddingXEm * fontPx;
	const padY = style.boxPaddingYEm * fontPx;
	const lines = Math.max(1, lineCount);
	const height = lines * style.lineHeight * fontPx + 2 * padY;
	const width = textWidthPx + 2 * padX;
	const radius = Math.min(style.boxRadiusEm * fontPx, height / 2);
	return { width, height, radius, padX, padY };
}
