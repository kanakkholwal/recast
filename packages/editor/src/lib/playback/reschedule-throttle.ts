/** Latest-wins, so the picture and the sound both land where the user let go. */
export type RescheduleDecision =
	| { act: "now" }
	| { act: "defer"; afterMs: number }
	| { act: "coalesce" };

/**
 * Minimum gap between audio reschedules, in ms. Matches `SEEK_MIN_INTERVAL_MS`
 * in `@recast/media`'s playback source: video and audio must chase a scrub at
 * the same rate or they visibly diverge.
 */
export const RESCHEDULE_MIN_INTERVAL_MS = 50;

/**
 * Whether a reschedule request may run now, must wait, or is already covered by
 * a pending one.
 *
 * A reschedule tears the audio graph down and back up — stop every source node,
 * abort the in-flight decode, re-anchor, restart the top-up timer. A drag emits
 * one request per pointer move, so unthrottled it rebuilds the graph over a
 * hundred times a second and every decode is aborted before it can produce
 * sound. That is audible as breakup, and it desynchronises from the picture,
 * which throttles.
 *
 * Pure so the policy is testable without an `AudioContext`.
 */
export function rescheduleDecision(
	nowMs: number,
	lastRunMs: number,
	timerPending: boolean,
	minIntervalMs = RESCHEDULE_MIN_INTERVAL_MS,
): RescheduleDecision {
	if (timerPending) return { act: "coalesce" };
	const sinceMs = nowMs - lastRunMs;
	if (sinceMs >= minIntervalMs) return { act: "now" };
	return { act: "defer", afterMs: minIntervalMs - sinceMs };
}
