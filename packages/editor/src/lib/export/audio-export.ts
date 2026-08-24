/**
 * Source audio for the browser export. Takes only the recording's own track and
 * warps it onto the OUTPUT timeline with the same regions the preview plays —
 * trim, cuts and per-segment speed — so the two can't drift apart.
 *
 * Speed is time-stretched pitch-preserving (see `timeStretch`), matching the
 * preview and the FFmpeg export's `atempo`.
 */

import type { Region } from "@recast/media";
import { buildTimeMap } from "../timeline/time-map";

export { resampleLinear, timeStretch } from "../playback/time-stretch";

/** One contiguous stretch of source audio and where it lands in the output. */
export interface AudioSpan {
	sourceStart: number;
	sourceEnd: number;
	outputStart: number;
	outputEnd: number;
	rate: number;
}

const EPS = 1e-6;

/**
 * Lay the kept regions end to end on the output timeline.
 *
 * A projection of `buildTimeMap`, not a second implementation: this used to lay
 * out the output axis itself, which made it one of several places that had to
 * agree about what speed does to duration.
 */
export function planAudioSpans(regions: readonly Region[]): AudioSpan[] {
	const map = buildTimeMap(
		regions.map((r) => ({
			origStart: r.start,
			origEnd: r.end,
			speed: r.speed ?? 1,
		})),
	);
	return map.spans.map((s) => ({
		sourceStart: s.origStart,
		sourceEnd: s.origEnd,
		outputStart: s.outStart,
		outputEnd: s.outEnd,
		rate: s.speed,
	}));
}

/** Total output seconds the planned spans occupy. */
export function audioOutputDuration(spans: readonly AudioSpan[]): number {
	return spans.length === 0 ? 0 : spans[spans.length - 1].outputEnd;
}

/**
 * Master fade envelope, applied in place over the whole output. Mirrors the
 * preview's fade node so an exported clip opens and closes the same way.
 */
export function applyFade(
	channel: Float32Array,
	sampleRate: number,
	totalSec: number,
	fadeInSec: number,
	fadeOutSec: number,
	offsetSec = 0,
): void {
	const inSamples = Math.max(0, Math.round(fadeInSec * sampleRate));
	const outSamples = Math.max(0, Math.round(fadeOutSec * sampleRate));
	const totalSamples = Math.max(0, Math.round(totalSec * sampleRate));
	const base = Math.round(offsetSec * sampleRate);
	for (let i = 0; i < channel.length; i++) {
		const abs = base + i;
		let gain = 1;
		if (inSamples > 0 && abs < inSamples) gain = abs / inSamples;
		const fromEnd = totalSamples - abs;
		if (outSamples > 0 && fromEnd <= outSamples) {
			gain = Math.min(gain, Math.max(0, fromEnd / outSamples));
		}
		if (gain !== 1) channel[i] *= gain;
	}
}

/** Master volume, applied in place. 0 mutes; 1 is unity. */
export function applyGain(channel: Float32Array, gain: number): void {
	if (Math.abs(gain - 1) < EPS) return;
	for (let i = 0; i < channel.length; i++) channel[i] *= gain;
}
