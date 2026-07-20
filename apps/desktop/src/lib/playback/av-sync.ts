/**
 * Pure A/V drift policy for the MediaBunny path, where the picture clock is a
 * wall-clock integrator and audio runs on the sound card's clock. Two crystals
 * means they drift apart over a long take with nothing to stop them.
 *
 * Audio is the master: a one-off picture correction is far less noticeable
 * than a gap or pitch artefact in the audio. `av-drift.ts` is the equivalent
 * for the legacy `<audio>`-element path and is not interchangeable.
 */

/**
 * Correct only past the point drift becomes perceptible. ITU-R BT.1359 puts
 * detectability near 45 ms audio-lead / 125 ms lag; 60 ms sits inside both and
 * is loose enough that normal jitter doesn't cause constant re-anchoring.
 */
export const AV_RESYNC_THRESHOLD_SEC = 0.06;

export interface AvSyncInput {
	/** Picture clock position, output-time seconds. */
	videoTime: number;
	/** Audio clock position, output-time seconds, or null when not scheduled. */
	audioTime: number | null;
	/** Whether playback is running; paused clocks can't drift. */
	playing: boolean;
	thresholdSec?: number;
}

export interface AvSyncDecision {
	/** Re-anchor the picture clock onto `target`. */
	resync: boolean;
	/** Output time to re-anchor to; only meaningful when `resync`. */
	target: number;
	/** Signed drift (video − audio) in seconds; 0 when unmeasurable. */
	driftSec: number;
}

/** Decide whether the picture clock should be pulled back onto the audio clock. */
export function resolveAvSync(input: AvSyncInput): AvSyncDecision {
	const { videoTime, audioTime, playing } = input;
	const threshold = input.thresholdSec ?? AV_RESYNC_THRESHOLD_SEC;
	if (!playing || audioTime === null || !Number.isFinite(audioTime)) {
		return { resync: false, target: videoTime, driftSec: 0 };
	}
	const driftSec = videoTime - audioTime;
	if (Math.abs(driftSec) <= threshold) {
		return { resync: false, target: videoTime, driftSec };
	}
	return { resync: true, target: audioTime, driftSec };
}
