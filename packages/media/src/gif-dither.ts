/**
 * Ordered dithering for GIF output.
 *
 * gifenc has no dither of its own, so a 256-colour palette applied straight to
 * a gradient produces hard bands: the "cheap GIF" look. Nudging each pixel by a
 * sub-step amount before the palette lookup trades those bands for fine noise
 * the eye reads as a smooth ramp.
 *
 * Ordered (Bayer) rather than Floyd-Steinberg on purpose: it is a single pass
 * with no neighbour search, so it runs per frame without stalling the worker,
 * and being position-based rather than error-propagating it stays stable
 * between frames instead of crawling.
 */

/** 8x8 Bayer matrix, normalised to -0.5..0.5 once at module load. */
const BAYER_8 = [
	[0, 32, 8, 40, 2, 34, 10, 42],
	[48, 16, 56, 24, 50, 18, 58, 26],
	[12, 44, 4, 36, 14, 46, 6, 38],
	[60, 28, 52, 20, 62, 30, 54, 22],
	[3, 35, 11, 43, 1, 33, 9, 41],
	[51, 19, 59, 27, 49, 17, 57, 25],
	[15, 47, 7, 39, 13, 45, 5, 37],
	[63, 31, 55, 23, 61, 29, 53, 21],
];

const THRESHOLD = new Float32Array(64);
for (let y = 0; y < 8; y++) {
	for (let x = 0; x < 8; x++) {
		THRESHOLD[y * 8 + x] = (BAYER_8[y][x] + 0.5) / 64 - 0.5;
	}
}

/**
 * Returns a dithered copy of `rgba`. `strength` is the size of the nudge in
 * 0-255 units; roughly the quantiser's step for the palette in play. Alpha is
 * left alone: perturbing it would fringe the transparent edges.
 *
 * The input is not mutated, so the caller can still build the palette from the
 * clean pixels and only dither the mapping.
 */
export function orderedDither(
	rgba: Uint8Array | Uint8ClampedArray,
	width: number,
	height: number,
	strength = 8,
): Uint8ClampedArray {
	const out = new Uint8ClampedArray(rgba.length);
	out.set(rgba);
	if (strength <= 0) return out;

	for (let y = 0; y < height; y++) {
		const row = (y & 7) * 8;
		for (let x = 0; x < width; x++) {
			const bias = THRESHOLD[row + (x & 7)] * strength;
			const i = (y * width + x) * 4;
			out[i] = (rgba[i] as number) + bias;
			out[i + 1] = (rgba[i + 1] as number) + bias;
			out[i + 2] = (rgba[i + 2] as number) + bias;
		}
	}
	return out;
}

/**
 * Dither strength for a palette size. Fewer colours means coarser steps, which
 * need a bigger nudge to break up; 256 colours barely bands at all.
 */
export function ditherStrengthFor(maxColors: number): number {
	if (maxColors <= 32) return 24;
	if (maxColors <= 64) return 16;
	if (maxColors <= 128) return 11;
	return 8;
}
