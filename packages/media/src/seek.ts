/**
 * Seek helpers shared by the editor's preview pipeline.
 *
 * These are pure functions over the cut list. They run on the main thread
 * and help the rAF loop schedule work; they do not talk to the worker.
 */

interface CutRange {
	/** Original-recording seconds. */
	start: number;
	end: number;
}

/**
 * Snap `seconds` (output clock) to a safe target on the original-recording
 * clock. If `seconds` falls inside a removed cut, return the cut's end (the
 * first kept frame after the cut); else return `seconds` unchanged.
 *
 * The editor's preview loop calls this when releasing a scrub so we land on
 * a frame the pipeline can actually show.
 */
export function snapToSeekTarget(seconds: number, cuts: ReadonlyArray<CutRange>): number {
	for (const cut of cuts) {
		if (seconds >= cut.start && seconds < cut.end) {
			return cut.end;
		}
	}
	return seconds;
}

/**
 * The next upcoming cut crossing within `lookaheadSeconds` of `seconds`
 * (output clock), or `null` if none. Used by the editor's preview loop to
 * schedule the "scout" pre-decode of the post-cut frame.
 *
 * Cuts whose start is `> seconds` and `<= seconds + lookahead` qualify.
 * Cuts fully in the past are ignored.
 */
export function nextCutWithin(
	seconds: number,
	lookahead: number,
	cuts: ReadonlyArray<CutRange>,
): CutRange | null {
	const horizon = seconds + lookahead;
	for (const cut of cuts) {
		if (cut.start > seconds && cut.start <= horizon) {
			return cut;
		}
	}
	return null;
}
