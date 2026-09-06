/** DraggableValue pure helpers: drag math + parse/format, kept testable. */

export interface DragModifiers {
	coarse?: boolean;
	fine?: boolean;
}

/** Movement (px) a pointer must travel before a press counts as a drag. */
export const DRAG_THRESHOLD_PX = 3;

export function clampValue(
	v: number,
	min = Number.NEGATIVE_INFINITY,
	max = Number.POSITIVE_INFINITY,
): number {
	return Math.min(max, Math.max(min, v));
}

/** Value delta for a horizontal drag: one step per px, Shift ×10, Alt ×0.1. */
export function dragDelta(dx: number, step: number, mods: DragModifiers = {}): number {
	const scale = mods.coarse ? 10 : mods.fine ? 0.1 : 1;
	return dx * step * scale;
}

/** Parse typed input; garbage keeps `fallback` so a stray keystroke never zeroes a field. */
export function parseInputValue(text: string, fallback: number): number {
	const parsed = Number.parseFloat(text);
	return Number.isNaN(parsed) ? fallback : parsed;
}

export function formatValue(v: number, decimals: number): string {
	return v.toFixed(decimals);
}
