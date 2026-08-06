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

/** Compact elapsed time: `45s`, or `1m 05s` past a minute. */
export function formatElapsed(ms: number): string {
	const s = Math.floor(ms / 1000);
	if (s < 60) return `${s}s`;
	return `${Math.floor(s / 60)}m ${s % 60}s`;
}

/**
 * Export ETA from elapsed × (1 − pct) / pct. Null until it's meaningful: no
 * real progress yet, finalising, below 10 %, or under 250 ms elapsed.
 */
export function exportEtaMs(args: {
	hasProgress: boolean;
	finalizing: boolean;
	progress: number;
	now: number;
	startedAt: number;
}): number | null {
	if (!args.hasProgress || args.finalizing) return null;
	const pct = args.progress;
	if (pct < 10) return null;
	const elapsed = args.now - args.startedAt;
	if (elapsed < 250) return null;
	return (elapsed * (100 - pct)) / pct;
}

/**
 * Rough time-remaining label for uploads/transfers, e.g. `~45s left`,
 * `~2m 10s left`. Rounds up so it never reads 0 while work remains; under a
 * second reads "almost done".
 */
export function etaLabel(secondsRemaining: number): string {
	const t = safe(secondsRemaining);
	if (t < 1) return "almost done";
	if (t < 60) return `~${Math.ceil(t)}s left`;
	const m = Math.floor(t / 60);
	const s = Math.round(t % 60);
	return s > 0 ? `~${m}m ${s}s left` : `~${m}m left`;
}
