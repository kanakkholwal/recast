/** Colour parsing for the WebGL preview. Store-free (gradient packing lives in `gradient.logic.ts`). */

/**
 * `#rrggbb`/`#rrggbbaa` → normalised `[r, g, b, a]` (0..1). Falls back to the
 * `#111111` background colour (with `alpha`) on malformed input.
 */
export function hexToRgba(hex: string, alpha = 1): [number, number, number, number] {
	const s = hex.trim().replace(/^#/, "");
	if (s.length < 6) return [17 / 255, 17 / 255, 17 / 255, alpha];
	const r = parseInt(s.slice(0, 2), 16) / 255;
	const g = parseInt(s.slice(2, 4), 16) / 255;
	const b = parseInt(s.slice(4, 6), 16) / 255;
	const a = s.length >= 8 ? parseInt(s.slice(6, 8), 16) / 255 : alpha;
	return [r, g, b, a];
}
