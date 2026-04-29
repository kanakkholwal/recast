// Pure evaluators for annotation timing and the editor's zoom transform.
//
// Extracted from AnnotationOverlay.svelte and TextAnnotationLayer.svelte (where
// they were duplicated). Owning a single copy means a regression in the math
// can be caught with a unit test and only one render path needs the fix.

import { bezierY, type Easing } from "$lib/easing/cubic-bezier";
import type { Annotation } from "$lib/stores/editor-store.svelte";

export interface ZoomRegionLike {
	start: number;
	end: number;
	scale: number;
	rampIn: number;
	rampOut: number;
	easeIn: Easing;
	easeOut: Easing;
	centerX?: number;
	centerY?: number;
}

export interface ZoomTransform {
	scale: number;
	cx: number;
	cy: number;
}

const IDENTITY: ZoomTransform = { scale: 1, cx: 0.5, cy: 0.5 };

/**
 * Current zoom transform at playback time `t`. Mirrors the WebGL preview
 * shader's evaluation so overlays land on the same pixels as the rendered
 * frame.
 */
export function evalZoom(zoomRegions: ZoomRegionLike[], t: number): ZoomTransform {
	for (const r of zoomRegions) {
		if (t <= r.start || t >= r.end) continue;
		const duration = Math.max(0, r.end - r.start);
		const half = duration * 0.5;
		const rampIn = Math.min(Math.max(0, r.rampIn), half);
		const rampOut = Math.min(Math.max(0, r.rampOut), half);
		const holdStart = r.start + rampIn;
		const holdEnd = r.end - rampOut;
		const cxTarget = r.centerX ?? 0.5;
		const cyTarget = r.centerY ?? 0.5;
		let phase: number;
		let curve: Easing;
		let atHold = false;
		if (t < holdStart) {
			phase = rampIn > 0 ? (t - r.start) / rampIn : 1;
			curve = r.easeIn;
		} else if (t > holdEnd) {
			phase = rampOut > 0 ? (r.end - t) / rampOut : 1;
			curve = r.easeOut;
		} else {
			atHold = true;
			phase = 1;
			curve = r.easeIn;
		}
		phase = Math.max(0, Math.min(1, phase));
		const eased = atHold ? 1 : bezierY(curve, phase);
		return {
			scale: 1 + (r.scale - 1) * eased,
			cx: 0.5 + (cxTarget - 0.5) * eased,
			cy: 0.5 + (cyTarget - 0.5) * eased,
		};
	}
	return IDENTITY;
}

/**
 * Annotation opacity at time `t`, applying split-ramp fade-in / fade-out
 * (the same semantics as Focus). Multiplied by the per-annotation `opacity`
 * (v2; defaults to 1).
 */
export function evalOpacity(a: Annotation, t: number): number {
	if (t <= a.start || t >= a.end) return 0;
	const dur = Math.max(0, a.end - a.start);
	const half = dur * 0.5;
	const rampIn = Math.min(Math.max(0, a.rampIn), half);
	const rampOut = Math.min(Math.max(0, a.rampOut), half);
	const holdStart = a.start + rampIn;
	const holdEnd = a.end - rampOut;

	let phase: number;
	let curve: Easing;
	let raw: number;
	if (t < holdStart) {
		phase = rampIn > 0 ? (t - a.start) / rampIn : 1;
		curve = a.easeIn;
		phase = Math.max(0, Math.min(1, phase));
		raw = Math.max(0, Math.min(1, bezierY(curve, phase)));
	} else if (t > holdEnd) {
		phase = rampOut > 0 ? (a.end - t) / rampOut : 1;
		curve = a.easeOut;
		phase = Math.max(0, Math.min(1, phase));
		raw = Math.max(0, Math.min(1, bezierY(curve, phase)));
	} else {
		raw = 1;
	}

	const master = a.opacity ?? 1;
	return raw * Math.max(0, Math.min(1, master));
}
