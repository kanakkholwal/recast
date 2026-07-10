// Pure helpers extracted from Timeline.svelte so subviews share them and they stay unit-testable.

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

// At least 2 frames so a trimmed range is never sub-frame.
export function minClipDuration(fps: number): number {
	return 2 * frameStep(fps);
}

// SMPTE-style HH:MM:SS:FF (MM:SS:FF for clips < 1 hour).
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

// smpte: HH:MM:SS:FF · seconds: M:SS.cs · frames: Nf
export type TimeMode = "smpte" | "seconds" | "frames";

export function formatFrames(time: number, fps: number): string {
	const frames = Math.max(0, Math.round(time * fps));
	return `${frames}f`;
}

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

export interface TimeMarker {
	time: number;
	label: string;
	emphasis: boolean;
}

// Major ruler labels: interval picked to keep ~50px between labels.
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

// Filled SVG envelope path for an audio waveform, built in output-pixel space
// (each bucket at `xOf(bucketTime)`) so buckets inside a removed cut collapse
// onto the seam. `range` clips to a kept window (in/out); null keeps everything.
// `amp` is the peak half-height; `mid` is height/2.
export function buildWaveformPath(p: {
	waveform: ReadonlyArray<number>;
	duration: number;
	xOf: (t: number) => number;
	height: number;
	amp: number;
	range?: { start: number; end: number } | null;
}): string {
	const w = p.waveform;
	const n = w.length;
	if (n < 2 || p.duration <= 0) return "";
	const mid = p.height / 2;
	const kept: number[] = [];
	for (let i = 0; i < n; i++) {
		if (p.range) {
			const t = (i / n) * p.duration;
			if (t < p.range.start - 0.001 || t > p.range.end + 0.001) continue;
		}
		kept.push(i);
	}
	if (kept.length < 2) return "";
	const xAt = (i: number) => p.xOf((i / n) * p.duration).toFixed(2);
	let d = `M ${xAt(kept[0])} ${mid}`;
	for (const i of kept) d += ` L ${xAt(i)} ${(mid - w[i] * p.amp).toFixed(2)}`;
	for (let k = kept.length - 1; k >= 0; k--) {
		const i = kept[k];
		d += ` L ${xAt(i)} ${(mid + w[i] * p.amp).toFixed(2)}`;
	}
	return `${d} Z`;
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
