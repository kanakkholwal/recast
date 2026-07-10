/**
 * FocusPanel zoom maths: scale at a given time (ramp-in → hold → ramp-out), the
 * sparkline path that visualises it, and a region's max ramp length.
 */

import type { Easing } from "$lib/easing/cubic-bezier";
import type { ZoomRegion } from "$lib/stores/editor-store.svelte";

/**
 * Time bounds for a new zoom region parked at `currentTime`: a short pre-roll
 * before and a hold after, clamped to the trimmed clip. Null when there's no
 * clip to zoom (the focus point / scale are resolved separately).
 */
export function computeNewZoomBounds(
	duration: number,
	trimStart: number,
	trimEnd: number,
	currentTime: number,
): { start: number; end: number } | null {
	if (duration <= 0) return null;
	const clipEnd = trimEnd || duration;
	const start = Math.max(trimStart, currentTime - 0.35);
	const end = Math.min(clipEnd, Math.max(start + 0.8, currentTime + 0.85));
	return { start, end };
}

/**
 * Longest a single ramp can be: half the span (so in+out can't overlap). Widened
 * past ZoomRegion so annotation fades (same half-span rule) can reuse it.
 */
export function regionMaxRamp(r: { start: number; end: number }): number {
	return Math.max(0, (r.end - r.start) * 0.5);
}

/**
 * Zoom scale at absolute time `t`: 1 outside the region, eased ramp up to
 * `r.scale` over `rampIn`, hold, then eased ramp back down over `rampOut`.
 */
export function scaleAt(r: ZoomRegion, t: number): number {
	if (t <= r.start || t >= r.end) return 1;
	const duration = Math.max(0, r.end - r.start);
	const half = duration * 0.5;
	const rampIn = Math.min(Math.max(0, r.rampIn), half);
	const rampOut = Math.min(Math.max(0, r.rampOut), half);
	const holdStart = r.start + rampIn;
	const holdEnd = r.end - rampOut;
	let phase: number;
	let curve: Easing;
	if (t < holdStart) {
		phase = rampIn > 0 ? (t - r.start) / rampIn : 1;
		curve = r.easeIn;
	} else if (t > holdEnd) {
		phase = rampOut > 0 ? (r.end - t) / rampOut : 1;
		curve = r.easeOut;
	} else {
		return r.scale;
	}
	phase = Math.max(0, Math.min(1, phase));
	// Low-budget x→y approximation (polynomial-in-t with t ≈ x). Indistinguishable
	// at sparkline resolution; avoids pulling in the full Newton-Raphson solver.
	const a = 1 - 3 * curve.y2 + 3 * curve.y1;
	const b = 3 * curve.y2 - 6 * curve.y1;
	const c = 3 * curve.y1;
	const s = ((a * phase + b) * phase + c) * phase;
	return 1 + (r.scale - 1) * s;
}

/**
 * SVG path for a region's zoom envelope across a `w × h` box: a normalised
 * 1.0 → scale → 1.0 curve sampled at 41 points.
 */
export function sparklinePath(r: ZoomRegion, w: number, h: number): string {
	const duration = Math.max(0.001, r.end - r.start);
	const maxScale = Math.max(r.scale, 1.0);
	const normScale = (s: number) =>
		maxScale === 1 ? 1 : (s - 1) / (maxScale - 1);
	const samples: Array<[number, number]> = [];
	const N = 40;
	for (let i = 0; i <= N; i++) {
		const t = (i / N) * duration;
		const absT = r.start + t;
		const s = scaleAt(r, absT);
		const x = (t / duration) * w;
		const y = h - normScale(s) * h * 0.9 - 1;
		samples.push([x, y]);
	}
	return samples
		.map(([x, y], i) => `${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`)
		.join(" ");
}
