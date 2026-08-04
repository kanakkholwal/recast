/**
 * Packs the store's gradient string into the flat uniform arrays the WebGL
 * preview shader consumes. Shares the store's gradient parser with the picker
 * and the Rust rasteriser so preview === export.
 */

import {
	MAX_GRADIENT_STOPS,
	parseGradient,
} from "$lib/stores/editor-store.svelte";
import { hexToRgba } from "./color.logic";

/**
 * Pack a gradient string into the shader's flat uniform arrays. Stops are
 * sorted, padded to MAX_GRADIENT_STOPS (extra slots repeat the last stop so the
 * shader's clamp is a no-op), positions normalised to 0..1, and the CSS-degree
 * angle converted to radians.
 */
export function buildGradientUniforms(value: string): {
	colors: Float32Array;
	positions: Float32Array;
	count: number;
	angleRad: number;
} {
	const spec = parseGradient(value || "");
	const sorted = [...spec.stops].sort((a, b) => a.pos - b.pos);
	const count = Math.min(Math.max(sorted.length, 2), MAX_GRADIENT_STOPS);
	const colors = new Float32Array(MAX_GRADIENT_STOPS * 4);
	const positions = new Float32Array(MAX_GRADIENT_STOPS);
	for (let i = 0; i < MAX_GRADIENT_STOPS; i++) {
		const stop = sorted[Math.min(i, sorted.length - 1)];
		const [r, g, b, a] = hexToRgba(stop.color);
		colors[i * 4] = r;
		colors[i * 4 + 1] = g;
		colors[i * 4 + 2] = b;
		colors[i * 4 + 3] = a;
		positions[i] = Math.min(1, Math.max(0, stop.pos / 100));
	}
	return { colors, positions, count, angleRad: (spec.angle * Math.PI) / 180 };
}
