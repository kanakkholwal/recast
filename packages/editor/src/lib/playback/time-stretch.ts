const DEFAULT_SAMPLE_RATE = 48_000;
const FRAME_SEC = 0.04;
const SEARCH_SEC = 0.004;
const MIN_FRAME = 32;
const EPS = 1e-6;
/** Correlation is the hot loop; every 2nd sample tracks the peak just as well. */
const CORR_STRIDE = 2;

/**
 * Resample by `rate` with linear interpolation: `rate` 2 halves the length.
 * Shifts pitch, so it is only the fallback for fragments too short to stretch.
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

const hannCache = new Map<number, Float32Array>();

function hann(length: number): Float32Array {
	const cached = hannCache.get(length);
	if (cached) return cached;
	const w = new Float32Array(length);
	for (let i = 0; i < length; i++) w[i] = 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / length);
	hannCache.set(length, w);
	return w;
}

/**
 * Offset within `±radius` of `ideal` whose overlap region best continues `ref`.
 * This is the WSOLA search: it keeps successive frames phase-aligned, which is
 * what stops the metallic doubling a naive overlap-add produces.
 */
function bestOffset(
	input: Float32Array,
	ideal: number,
	ref: number,
	overlap: number,
	radius: number,
	maxStart: number,
): number {
	let bestPos = Math.min(Math.max(ideal, 0), maxStart);
	let bestScore = Number.NEGATIVE_INFINITY;
	for (let delta = -radius; delta <= radius; delta++) {
		const pos = ideal + delta;
		if (pos < 0 || pos > maxStart) continue;
		let dot = 0;
		let energy = 0;
		for (let i = 0; i < overlap; i += CORR_STRIDE) {
			const a = input[pos + i];
			dot += a * input[ref + i];
			energy += a * a;
		}
		// Normalising by the candidate's own energy stops the search from always
		// picking the loudest window instead of the best-aligned one.
		const score = dot / Math.sqrt(energy + EPS);
		if (score > bestScore) {
			bestScore = score;
			bestPos = pos;
		}
	}
	return bestPos;
}

/**
 * Change playback speed without changing pitch (WSOLA overlap-add).
 * `rate` 2 halves the duration; the FFmpeg export expresses the same warp as
 * `atempo`, so preview, browser export and Rust export stay in agreement.
 */
export function timeStretch(
	input: Float32Array,
	rate: number,
	sampleRate: number = DEFAULT_SAMPLE_RATE,
): Float32Array {
	if (!(rate > 0) || !Number.isFinite(rate) || Math.abs(rate - 1) < EPS) return input;

	const outLength = Math.max(0, Math.floor(input.length / rate));
	if (outLength === 0) return new Float32Array(0);

	// Frame must fit the input twice over for the search to have anywhere to go;
	// below that a fragment is short enough that resampling is inaudible.
	let frame = Math.min(Math.round(FRAME_SEC * sampleRate), Math.floor(input.length / 4) * 2);
	frame -= frame % 2;
	if (frame < MIN_FRAME) return resampleLinear(input, rate);

	const synHop = frame / 2;
	const anaHop = synHop * rate;
	const radius = Math.max(1, Math.round(SEARCH_SEC * sampleRate));
	const maxStart = input.length - frame;
	const window = hann(frame);

	const out = new Float32Array(outLength);
	const norm = new Float32Array(outLength);
	let prevChosen = 0;

	for (let s = 0; s * synHop < outLength; s++) {
		const synPos = s * synHop;
		const ideal = Math.round(s * anaHop);
		const chosen =
			s === 0
				? Math.min(Math.max(ideal, 0), maxStart)
				: bestOffset(
						input,
						ideal,
						Math.min(prevChosen + synHop, maxStart),
						synHop,
						radius,
						maxStart,
					);
		const span = Math.min(frame, outLength - synPos);
		for (let i = 0; i < span; i++) {
			const w = window[i];
			out[synPos + i] += input[chosen + i] * w;
			norm[synPos + i] += w;
		}
		prevChosen = chosen;
	}

	// Hann at 50% hop sums to unity in the steady state but tapers at both
	// edges; dividing by the actual window sum keeps gain flat end to end.
	for (let i = 0; i < outLength; i++) {
		if (norm[i] > EPS) out[i] /= norm[i];
	}
	return out;
}
