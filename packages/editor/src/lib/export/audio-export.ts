/**
 * Source audio for the browser export. Takes only the recording's own track and
 * warps it onto the OUTPUT timeline with the same regions the preview plays —
 * trim, cuts and per-segment speed — so the two can't drift apart.
 *
 * Speed resamples rather than time-stretches, which shifts pitch. That is
 * deliberate: the preview does the same (`AudioBufferSourceNode.playbackRate`),
 * and matching it is the parity requirement.
 */

import type { Region } from "@recast/media";

/** One contiguous stretch of source audio and where it lands in the output. */
export interface AudioSpan {
	sourceStart: number;
	sourceEnd: number;
	outputStart: number;
	outputEnd: number;
	rate: number;
}

const EPS = 1e-6;

/** Lay the kept regions end to end on the output timeline. */
export function planAudioSpans(regions: readonly Region[]): AudioSpan[] {
	const spans: AudioSpan[] = [];
	let outCursor = 0;
	for (const r of regions) {
		const rate = r.speed && r.speed > 0 ? r.speed : 1;
		const srcDur = Math.max(0, r.end - r.start);
		if (srcDur <= EPS) continue;
		const outDur = srcDur / rate;
		spans.push({
			sourceStart: r.start,
			sourceEnd: r.end,
			outputStart: outCursor,
			outputEnd: outCursor + outDur,
			rate,
		});
		outCursor += outDur;
	}
	return spans;
}

/** Total output seconds the planned spans occupy. */
export function audioOutputDuration(spans: readonly AudioSpan[]): number {
	return spans.length === 0 ? 0 : spans[spans.length - 1].outputEnd;
}

/**
 * Resample `input` by `rate` with linear interpolation: `rate` 2 halves the
 * length (plays twice as fast). Mirrors `playbackRate`, pitch shift included.
 */
export function resampleLinear(input: Float32Array, rate: number): Float32Array {
	if (!(rate > 0) || Math.abs(rate - 1) < EPS) return input;
	const outLength = Math.max(0, Math.floor(input.length / rate));
	const out = new Float32Array(outLength);
	for (let i = 0; i < outLength; i++) {
		const pos = i * rate;
		const i0 = Math.floor(pos);
		const i1 = Math.min(i0 + 1, input.length - 1);
		const frac = pos - i0;
		out[i] = input[i0] * (1 - frac) + input[i1] * frac;
	}
	return out;
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
