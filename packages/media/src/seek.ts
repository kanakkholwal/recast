/**
 * Seek helpers — utilities for resolving the editor's "current time" into a
 * frame on the original-recording clock (taking cuts into account) and
 * snapping to the nearest decodable keyframe. (PR-A: stub; PR-D lands the
 * real implementation.)
 */

/**
 * Snap `seconds` (on the OUTPUT clock) to the nearest decodable frame on the
 * ORIGINAL-recording clock, taking cuts into account. Used by the editor's
 * preview loop when the user releases a scrub: pick the frame we should
 * actually display.
 */
export function snapToSeekTarget(
	_seconds: number,
	_cuts: ReadonlyArray<{ start: number; end: number }>,
): number {
	throw new Error('snapToSeekTarget is not yet implemented — lands in PR-D');
}

/**
 * The next upcoming cut crossing within `lookaheadSeconds` of `seconds`
 * (output clock), or `null` if none. The editor uses this to schedule the
 * pre-cut frame pre-decode (the "scout decoder" pattern from the existing
 * `webcodecs-source.ts`).
 */
export function nextCutWithin(
	_seconds: number,
	_lookaheadSeconds: number,
	_cuts: ReadonlyArray<{ start: number; end: number }>,
): { start: number; end: number } | null {
	throw new Error('nextCutWithin is not yet implemented — lands in PR-D');
}
