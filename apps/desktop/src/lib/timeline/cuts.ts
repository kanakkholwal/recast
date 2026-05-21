/**
 * Timeline cut model + time mapping.
 *
 * A `TimelineCut` is a removed range expressed on the **original-recording**
 * timeline (the same coordinate space zoom regions and annotations use). The
 * exported / played-back result is the recording minus the union of all cuts.
 *
 * Because cuts remove time from the middle, mapping between original time and
 * output (post-cut) time is piecewise — these helpers own that arithmetic so
 * the playhead, scrubber, and preview all agree.
 */

export type CutSource = "silence" | "manual";

export interface TimelineCut {
	id: string;
	/** Start on the original-recording timeline (seconds). */
	start: number;
	/** End on the original-recording timeline (seconds). */
	end: number;
	source: CutSource;
}

/**
 * Sort cuts and merge any that overlap or touch, so every downstream
 * calculation can assume a clean, ordered, disjoint list.
 */
export function normalizeCuts(cuts: TimelineCut[]): TimelineCut[] {
	const valid = cuts
		.filter((c) => c.end > c.start)
		.sort((a, b) => a.start - b.start);
	const out: TimelineCut[] = [];
	for (const c of valid) {
		const last = out[out.length - 1];
		if (last && c.start <= last.end + 1e-4) {
			last.end = Math.max(last.end, c.end);
		} else {
			out.push({ ...c });
		}
	}
	return out;
}

/** Total seconds removed by all cuts. */
export function totalCutDuration(cuts: TimelineCut[]): number {
	return normalizeCuts(cuts).reduce((sum, c) => sum + (c.end - c.start), 0);
}

/** The cut containing `t` (original seconds), or null if `t` is kept. */
export function cutContaining(
	cuts: TimelineCut[],
	t: number,
): TimelineCut | null {
	for (const c of cuts) {
		if (t >= c.start && t < c.end) return c;
	}
	return null;
}

/**
 * Map an original-timeline time to output (post-cut) time. A time that falls
 * inside a cut collapses onto the cut's start.
 */
export function originalToOutput(cuts: TimelineCut[], t: number): number {
	let removed = 0;
	for (const c of normalizeCuts(cuts)) {
		if (c.end <= t) {
			removed += c.end - c.start;
		} else if (c.start < t) {
			removed += t - c.start;
			break;
		} else {
			break;
		}
	}
	return t - removed;
}

/** Map an output (post-cut) time back to an original-timeline time. */
export function outputToOriginal(cuts: TimelineCut[], t: number): number {
	let orig = t;
	for (const c of normalizeCuts(cuts)) {
		if (c.start <= orig) {
			orig += c.end - c.start;
		} else {
			break;
		}
	}
	return orig;
}

/**
 * True when `[start, end)` overlaps any cut — used to gate silence
 * suggestions that would bisect an existing zoom region or annotation.
 */
export function overlapsAny(
	ranges: Array<{ start: number; end: number }>,
	start: number,
	end: number,
): boolean {
	return ranges.some((r) => start < r.end && end > r.start);
}
