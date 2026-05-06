import type { Easing } from "$lib/easing/cubic-bezier";
import type { ZoomRegion } from "$lib/stores/editor-store.svelte";

// Pure helpers extracted from Timeline.svelte. Keeping these out of the
// Svelte component lets the timeline subviews share them without re-importing
// the orchestrator, and makes them unit-testable in isolation.

export function effectiveFps(metadataFps: number | undefined): number {
	const f = metadataFps ?? 0;
	return f > 0 ? f : 60;
}

export function quantizeToFrame(time: number, fps: number): number {
	return Math.round(time * fps) / fps;
}

export function frameStep(fps: number): number {
	return 1 / fps;
}

// Floor on the clip length: at least 2 frames so the trimmed range is never
// sub-frame. Scales naturally with fps (60fps → ~33ms; 30fps → ~66ms).
export function minClipDuration(fps: number): number {
	return 2 * frameStep(fps);
}

// SMPTE-style HH:MM:SS:FF (or MM:SS:FF for clips < 1 hour). Frame component
// is zero-padded so the readout has constant width.
export function formatTimecode(time: number, fps: number): string {
	const t = Math.max(0, time);
	const totalFrames = Math.round(t * fps);
	const frames = totalFrames % Math.round(fps);
	const totalSecs = Math.floor(totalFrames / Math.round(fps));
	const secs = totalSecs % 60;
	const mins = Math.floor(totalSecs / 60) % 60;
	const hours = Math.floor(totalSecs / 3600);
	const ff = String(frames).padStart(2, "0");
	const ss = String(secs).padStart(2, "0");
	const mm = String(mins).padStart(2, "0");
	return hours > 0
		? `${String(hours).padStart(2, "0")}:${mm}:${ss}:${ff}`
		: `${mm}:${ss}:${ff}`;
}

// Display modes the timeline supports. Stored as a discriminated string so
// it round-trips cleanly through component props.
//   smpte   — HH:MM:SS:FF (full editorial timecode)
//   seconds — M:SS.cs   (decimal seconds, useful at low zoom)
//   frames  — Nf        (raw frame count, useful for frame-precision work)
export type TimeMode = "smpte" | "seconds" | "frames";

export function formatFrames(time: number, fps: number): string {
	const frames = Math.max(0, Math.round(time * fps));
	return `${frames}f`;
}

// Single entry-point for all timeline labels. Sub-views call this with the
// active TimeMode so the format flips everywhere at once.
export function formatTimeByMode(
	time: number,
	mode: TimeMode,
	fps: number,
): string {
	switch (mode) {
		case "smpte":
			return formatTimecode(time, fps);
		case "seconds":
			return formatTime(time);
		case "frames":
			return formatFrames(time, fps);
	}
}

export function formatTime(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	const centiseconds = Math.floor((seconds % 1) * 100);
	return `${mins}:${secs.toString().padStart(2, "0")}.${centiseconds
		.toString()
		.padStart(2, "0")}`;
}

export function greatestCommonDivisor(a: number, b: number): number {
	let left = Math.abs(a);
	let right = Math.abs(b);
	while (right !== 0) {
		const next = left % right;
		left = right;
		right = next;
	}
	return left || 1;
}

// Approximate polynomial-in-t eval; indistinguishable from the real
// Newton-Raphson solve at sparkline resolution.
export function approxEaseY(easing: Easing, x: number): number {
	const a = 1 - 3 * easing.y2 + 3 * easing.y1;
	const b = 3 * easing.y2 - 6 * easing.y1;
	const c = 3 * easing.y1;
	return ((a * x + b) * x + c) * x;
}

// Path drawing 0..100 × 0..18 viewBox: the region's scale curve, normalised
// so peak scale reaches the top of the box. Shows the rampIn/hold/rampOut
// shape at a glance.
export function zoomSparklinePath(r: ZoomRegion): string {
	const duration = Math.max(0.001, r.end - r.start);
	const half = duration * 0.5;
	const rampIn = Math.min(Math.max(0, r.rampIn), half);
	const rampOut = Math.min(Math.max(0, r.rampOut), half);
	const holdStart = rampIn;
	const holdEnd = duration - rampOut;
	const peak = Math.max(r.scale, 1.0);
	const norm = (s: number) => (peak === 1 ? 0 : (s - 1) / (peak - 1));
	const W = 100;
	const H = 18;
	const pts: string[] = [];
	const N = 48;
	for (let i = 0; i <= N; i++) {
		const t = (i / N) * duration;
		let s = 1.0;
		if (t < holdStart) {
			const phase = rampIn > 0 ? t / rampIn : 1;
			s = 1 + (r.scale - 1) * approxEaseY(r.easeIn, phase);
		} else if (t > holdEnd) {
			const phase = rampOut > 0 ? (duration - t) / rampOut : 1;
			s = 1 + (r.scale - 1) * approxEaseY(r.easeOut, phase);
		} else {
			s = r.scale;
		}
		const x = (t / duration) * W;
		const y = H - norm(s) * (H - 2) - 1;
		pts.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
	}
	return pts.join(" ");
}

export interface TimeMarker {
	time: number;
	label: string;
	emphasis: boolean;
}

// Major ruler labels — interval picked to keep ~50px between labels.
export function buildTimeMarkers(
	duration: number,
	pixelsPerSecond: number,
): TimeMarker[] {
	if (duration <= 0) return [];
	const markers: TimeMarker[] = [];
	let interval = 1;
	if (pixelsPerSecond < 26) interval = 10;
	else if (pixelsPerSecond < 52) interval = 5;
	else if (pixelsPerSecond < 120) interval = 2;
	else if (pixelsPerSecond > 260) interval = 0.5;

	for (let t = 0; t <= duration + interval * 0.5; t += interval) {
		const mins = Math.floor(t / 60);
		const secs = Math.floor(t % 60);
		markers.push({
			time: t,
			label: `${mins}:${secs.toString().padStart(2, "0")}`,
			emphasis: Math.round(t) % (interval >= 2 ? interval * 2 : 2) === 0,
		});
	}
	return markers;
}

// Minor tick marks between labels.
export function buildMinorTicks(
	duration: number,
	pixelsPerSecond: number,
): number[] {
	if (duration <= 0) return [];
	const ticks: number[] = [];
	const interval =
		pixelsPerSecond > 180 ? 0.25 : pixelsPerSecond > 80 ? 0.5 : 1;
	for (let t = 0; t <= duration + interval * 0.5; t += interval) {
		ticks.push(t);
	}
	return ticks;
}
