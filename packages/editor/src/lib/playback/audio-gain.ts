/** Gain scalars for the preview mixer, kept in the same band the export uses. */

/** Highest gain the export will apply (`effective_audio_gain` in commands/editor.rs). */
export const MAX_GAIN = 4;

/** A 0-200% slider position as a linear gain scalar. */
export function gainFromPercent(percent: number): number {
	if (!Number.isFinite(percent)) return 0;
	return Math.max(0, Math.min(MAX_GAIN, percent / 100));
}

/** Final gain for one track: a mute on either level wins, else master x track. */
export function trackGain(
	masterPercent: number,
	trackPercent: number,
	masterMuted: boolean,
	trackMuted: boolean,
): number {
	if (masterMuted || trackMuted) return 0;
	return gainFromPercent(masterPercent) * gainFromPercent(trackPercent);
}
