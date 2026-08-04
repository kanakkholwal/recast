import type { HandleName } from "./hit";

/**
 * Pure geometry for constrained resizing/drawing of box + arrow annotations.
 * All positions are UV (0..1). The frame isn't square, so "square" and "45°"
 * are computed in visual pixels (UV × frame dimensions) and mapped back, so
 * they look right on screen regardless of aspect ratio.
 */

export type Box = { x: number; y: number; w: number; h: number };

export function isCornerHandle(h: HandleName): boolean {
	return h === "nw" || h === "ne" || h === "se" || h === "sw";
}

/** Constrain a signed box to a visual square: equal on-screen pixels. */
export function constrainSquare(
	w: number,
	h: number,
	frameW: number,
	frameH: number,
): { w: number; h: number } {
	const side = Math.max(Math.abs(w) * frameW, Math.abs(h) * frameH);
	return {
		w: (w < 0 ? -1 : 1) * (side / frameW),
		h: (h < 0 ? -1 : 1) * (side / frameH),
	};
}

/** Snap a free endpoint to the nearest 45° from an anchor, in visual space. */
export function constrain45(
	ax: number,
	ay: number,
	px: number,
	py: number,
	frameW: number,
	frameH: number,
): { x: number; y: number } {
	const dx = (px - ax) * frameW;
	const dy = (py - ay) * frameH;
	const step = Math.PI / 4;
	const ang = Math.round(Math.atan2(dy, dx) / step) * step;
	const len = Math.hypot(dx, dy);
	return {
		x: ax + (Math.cos(ang) * len) / frameW,
		y: ay + (Math.sin(ang) * len) / frameH,
	};
}

/**
 * Lock a corner resize to the starting box's aspect ratio. Preserving the UV
 * w:h ratio preserves the visual aspect too (the frame is fixed). The corner
 * opposite the dragged handle stays put.
 */
export function lockAspect(
	handle: HandleName,
	b: Box,
	nx: number,
	ny: number,
	nw: number,
	nh: number,
): { nx: number; ny: number; nw: number; nh: number } {
	if (b.w <= 0 || b.h <= 0) return { nx, ny, nw, nh };
	const aspect = b.w / b.h;
	let mw = Math.abs(nw);
	let mh = Math.abs(nh);
	if (mw / b.w >= mh / b.h) mh = mw / aspect;
	else mw = mh * aspect;
	const outW = (nw < 0 ? -1 : 1) * mw;
	const outH = (nh < 0 ? -1 : 1) * mh;
	const controlsLeft = handle === "nw" || handle === "w" || handle === "sw";
	const controlsTop = handle === "nw" || handle === "n" || handle === "ne";
	return {
		nx: controlsLeft ? b.x + b.w - outW : b.x,
		ny: controlsTop ? b.y + b.h - outH : b.y,
		nw: outW,
		nh: outH,
	};
}

/**
 * Center a box in UV sized to an image's aspect ratio, correcting for the frame
 * aspect so pixels aren't stretched. Targets ~40% of the frame; falls back to a
 * square when the natural size is unknown.
 */
export function fitImageBox(
	natural: { w: number; h: number } | null,
	frameAspect: number,
	target = 0.4,
): Box {
	let w = target;
	let h = target;
	if (natural && natural.w > 0 && natural.h > 0) {
		const ratio = natural.w / natural.h / frameAspect;
		if (ratio >= 1) {
			w = target;
			h = target / ratio;
		} else {
			h = target;
			w = target * ratio;
		}
	}
	return { x: (1 - w) / 2, y: (1 - h) / 2, w, h };
}
