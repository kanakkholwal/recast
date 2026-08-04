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

/**
 * How long the audio clock may sit still before we stop trusting it. A
 * suspended `AudioContext` freezes `currentTime`, and a master clock that never
 * advances drags the picture back onto the same instant every frame — an
 * audio fault that presents as a totally frozen video.
 */
export const AUDIO_STALL_LIMIT_SEC = 0.5;

export interface AvSyncInput {
	/** Picture clock position, output-time seconds. */
	videoTime: number;
	/** Audio clock position, output-time seconds, or null when not scheduled. */
	audioTime: number | null;
	/** Whether playback is running; paused clocks can't drift. */
	playing: boolean;
	thresholdSec?: number;
	/** Seconds the audio clock has been observed frozen — see {@link AudioStallMonitor}. */
	audioStalledSec?: number;
}

export interface AvSyncDecision {
	/** Re-anchor the picture clock onto `target`. */
	resync: boolean;
	/** Output time to re-anchor to; only meaningful when `resync`. */
	target: number;
	/** Signed drift (video − audio) in seconds; 0 when unmeasurable. */
	driftSec: number;
	/** Audio stopped advancing, so the picture is running unmastered. */
	audioStalled: boolean;
}

/**
 * Tracks whether the audio clock is actually moving. Deterministic given its
 * inputs — the caller supplies the timestamp — so it stays unit-testable.
 */
export class AudioStallMonitor {
	#lastAudioTime: number | null = null;
	#lastChangeMs = 0;

	/** Returns how long audio has been frozen, in seconds. */
	observe(audioTime: number | null, playing: boolean, nowMs: number): number {
		if (!playing || audioTime === null || !Number.isFinite(audioTime)) {
			this.reset();
			return 0;
		}
		if (this.#lastAudioTime === null || audioTime !== this.#lastAudioTime) {
			this.#lastAudioTime = audioTime;
			this.#lastChangeMs = nowMs;
			return 0;
		}
		return Math.max(0, (nowMs - this.#lastChangeMs) / 1000);
	}

	reset(): void {
		this.#lastAudioTime = null;
		this.#lastChangeMs = 0;
	}
}

/** Decide whether the picture clock should be pulled back onto the audio clock. */
export function resolveAvSync(input: AvSyncInput): AvSyncDecision {
	const { videoTime, audioTime, playing } = input;
	const threshold = input.thresholdSec ?? AV_RESYNC_THRESHOLD_SEC;
	if (!playing || audioTime === null || !Number.isFinite(audioTime)) {
		return { resync: false, target: videoTime, driftSec: 0, audioStalled: false };
	}
	const driftSec = videoTime - audioTime;
	// A dead clock is not a master. Let the picture run free rather than
	// pinning it to a timestamp that will never move again.
	if ((input.audioStalledSec ?? 0) > AUDIO_STALL_LIMIT_SEC) {
		return { resync: false, target: videoTime, driftSec, audioStalled: true };
	}
	if (Math.abs(driftSec) <= threshold) {
		return { resync: false, target: videoTime, driftSec, audioStalled: false };
	}
	return { resync: true, target: audioTime, driftSec, audioStalled: false };
}
