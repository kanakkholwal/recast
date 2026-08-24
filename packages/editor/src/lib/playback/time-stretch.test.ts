import { describe, expect, it } from "vitest";
import { resampleLinear } from "../export/audio-export";
import { timeStretch } from "./time-stretch";

const SAMPLE_RATE = 48_000;

function sine(freqHz: number, seconds: number, sampleRate = SAMPLE_RATE): Float32Array {
	const out = new Float32Array(Math.round(seconds * sampleRate));
	for (let i = 0; i < out.length; i++) out[i] = Math.sin((2 * Math.PI * freqHz * i) / sampleRate);
	return out;
}

/** Goertzel magnitude of `freqHz` in `signal` — no FFT dependency needed. */
function binMagnitude(signal: Float32Array, freqHz: number, sampleRate = SAMPLE_RATE): number {
	const k = (2 * Math.PI * freqHz) / sampleRate;
	const coeff = 2 * Math.cos(k);
	let s1 = 0;
	let s2 = 0;
	for (let i = 0; i < signal.length; i++) {
		const s0 = signal[i] + coeff * s1 - s2;
		s2 = s1;
		s1 = s0;
	}
	return Math.sqrt(s1 * s1 + s2 * s2 - coeff * s1 * s2) / signal.length;
}

/** Which of `candidates` carries the most energy. */
function dominantFreq(signal: Float32Array, candidates: number[]): number {
	let best = candidates[0];
	let bestMag = -1;
	for (const f of candidates) {
		const mag = binMagnitude(signal, f);
		if (mag > bestMag) {
			bestMag = mag;
			best = f;
		}
	}
	return best;
}

const BASE_HZ = 440;
const CANDIDATES = [220, 440, 880];

describe("resampleLinear (the pitch-shifting path this replaces)", () => {
	it("moves a 440Hz tone to 880Hz at 2x, which is the chipmunk defect", () => {
		const out = resampleLinear(sine(BASE_HZ, 1), 2);
		expect(dominantFreq(out, CANDIDATES)).toBe(880);
	});
});

describe("timeStretch", () => {
	it("returns the input untouched at 1x", () => {
		const input = sine(BASE_HZ, 0.5);
		expect(timeStretch(input, 1)).toBe(input);
	});

	it("keeps a 440Hz tone at 440Hz when sped up 2x", () => {
		const out = timeStretch(sine(BASE_HZ, 2), 2);
		expect(dominantFreq(out, CANDIDATES)).toBe(BASE_HZ);
	});

	it("keeps a 440Hz tone at 440Hz when slowed to 0.5x", () => {
		const out = timeStretch(sine(BASE_HZ, 1), 0.5);
		expect(dominantFreq(out, CANDIDATES)).toBe(BASE_HZ);
	});

	it("produces output length input.length / rate", () => {
		const input = sine(BASE_HZ, 2);
		for (const rate of [0.5, 1.25, 2, 4]) {
			const out = timeStretch(input, rate);
			const expected = Math.floor(input.length / rate);
			expect(Math.abs(out.length - expected)).toBeLessThanOrEqual(1);
		}
	});

	it("matches resampleLinear's output length so span planning is unchanged", () => {
		const input = sine(BASE_HZ, 1);
		for (const rate of [0.5, 2, 3]) {
			expect(timeStretch(input, rate).length).toBe(resampleLinear(input, rate).length);
		}
	});

	it("holds unity gain rather than doubling through the overlap-add", () => {
		const out = timeStretch(sine(BASE_HZ, 1), 2);
		let peak = 0;
		for (const v of out) peak = Math.max(peak, Math.abs(v));
		expect(peak).toBeGreaterThan(0.7);
		expect(peak).toBeLessThan(1.2);
	});

	it("emits no NaN or Infinity", () => {
		const out = timeStretch(sine(BASE_HZ, 1), 1.7);
		expect(out.every((v) => Number.isFinite(v))).toBe(true);
	});

	it("keeps silence silent", () => {
		const out = timeStretch(new Float32Array(SAMPLE_RATE), 2);
		expect(out.every((v) => v === 0)).toBe(true);
	});

	it("passes through degenerate rates instead of dividing by zero", () => {
		const input = sine(BASE_HZ, 0.1);
		expect(timeStretch(input, 0)).toBe(input);
		expect(timeStretch(input, Number.NaN)).toBe(input);
		expect(timeStretch(input, -2)).toBe(input);
	});

	it("handles input shorter than one analysis window", () => {
		const out = timeStretch(sine(BASE_HZ, 0.002), 2);
		expect(out.every((v) => Number.isFinite(v))).toBe(true);
	});
});
