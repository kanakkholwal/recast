/**
 * Pure timecode formatters for the editor panels. All take SECONDS, clamp
 * negatives to 0, and treat non-finite as 0 (NaN → zero clock, not `NaN:NaN`).
 * Minutes are never rolled into hours, because recordings are short.
 */

function safe(sec: number): number {
	return Number.isFinite(sec) ? Math.max(0, sec) : 0;
}

/** `M:SS`, e.g. `1:05`. */
export function clock(sec: number): string {
	const t = safe(sec);
	const m = Math.floor(t / 60);
	const s = Math.floor(t % 60);
	return `${m}:${s.toString().padStart(2, "0")}`;
}

/** `M:SS.cc` with centiseconds (truncated), e.g. `1:05.23`. */
export function clockCentis(sec: number): string {
	const t = safe(sec);
	const m = Math.floor(t / 60);
	const s = Math.floor(t % 60);
	const cs = Math.floor((t % 1) * 100);
	return `${m}:${s.toString().padStart(2, "0")}.${cs.toString().padStart(2, "0")}`;
}

/** `M:SS.d` with deciseconds, e.g. `1:05.4`. */
export function clockDecis(sec: number): string {
	const t = safe(sec);
	const m = Math.floor(t / 60);
	const rem = t - m * 60;
	return `${m}:${rem.toFixed(1).padStart(4, "0")}`;
}

/** Compact duration: `1.5s` at/above a second, else `500ms`. */
export function compactDuration(sec: number): string {
	return sec >= 1 ? `${sec.toFixed(1)}s` : `${Math.round(sec * 1000)}ms`;
}
