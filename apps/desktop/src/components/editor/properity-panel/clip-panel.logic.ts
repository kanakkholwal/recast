/** ClipPanel helpers: speed label and the float anchor-equality idiom. */

/** Format a speed multiplier for display ("1.5×"). */
export function fmtSpeed(s: number): string {
	return `${s}×`;
}

/**
 * Whether two second/anchor values are equal within float tolerance — segments
 * are matched by their original start time, and speeds compared to presets.
 */
export function anchorMatches(a: number, b: number): boolean {
	return Math.abs(a - b) < 1e-4;
}
