/**
 * Audio/video drift reconciliation for the legacy `<video>`+`<audio>` preview
 * path (the WebCodecs engine path owns its own clock and doesn't use this).
 *
 * The recording mp4 has no audio, so the `<video>` element is muted and sound
 * comes from separate `<audio>` elements slaved to the video clock. Under load
 * (e.g. recording Recast while previewing) the *video* decode stalls while the
 * audio keeps playing — so the audio runs ahead of the lagging picture.
 *
 * The old correction hard-seeked the audio *backward* to the stalled video
 * position, replaying the last slice repeatedly: an audible echo. Instead:
 *   - on an intentional jump (cut skip / user seek) → snap audio to the picture
 *   - when audio falls *behind* the picture → nudge audio forward to catch up
 *   - when the picture has stalled far *behind* the audio → advance the PICTURE
 *     to the audio (a brief visual skip — "buffering caught up"), never rewind
 *     the audio, so there is no echo and the lead can't accumulate
 *   - otherwise → tolerate the small drift (inaudible, no correction)
 */
export type AvDriftAction = "none" | "resync-audio" | "catch-picture";

export function reconcileAvDrift(params: {
	/** Current playback time of the `<audio>` element (seconds). */
	audioTime: number;
	/** Current time of the picture clock — `<video>`.currentTime (seconds). */
	pictureTime: number;
	/** True for an intentional jump (cut boundary, scrub, loop). */
	isJump: boolean;
	/** Drift past which a lagging audio element is nudged forward. */
	syncThreshold: number;
	/** Lead past which we advance the picture instead of leaving the gap. */
	maxLead: number;
}): AvDriftAction {
	const { audioTime, pictureTime, isJump, syncThreshold, maxLead } = params;
	// Intentional jump — snap audio to the new position in either direction.
	if (isJump) return "resync-audio";
	// `lead > 0` ⇒ audio is ahead of a lagging picture; `lead < 0` ⇒ audio behind.
	const lead = audioTime - pictureTime;
	// Audio fell behind the picture — nudge it forward to catch up.
	if (lead < -syncThreshold) return "resync-audio";
	// Picture stalled far behind the audio — move the picture, not the audio.
	if (lead > maxLead) return "catch-picture";
	// Small drift: leave it. Never rewind audio to a merely-stalled picture.
	return "none";
}
