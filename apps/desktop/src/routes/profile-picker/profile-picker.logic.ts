/** URL param + profile summary for the profile-picker window. */

import type { RecordingProfile } from "$lib/profiles";

export function parseSelectedParam(search: string): string | null {
	return new URLSearchParams(search).get("selected") ?? null;
}

/**
 * One-line capture summary chips for a profile (audio/mic/camera, or "Silent
 * capture" when nothing is on). Distinct from the profiles-page summary, this
 * shape favours the compact picker row.
 */
export function summarize(profile: RecordingProfile): string[] {
	const out: string[] = [];
	if (profile.systemAudio) out.push("Audio");
	if (profile.microphone)
		out.push(profile.micLabel ? `Mic: ${profile.micLabel}` : "Mic");
	if (profile.camera)
		out.push(profile.cameraLabel ? `Cam: ${profile.cameraLabel}` : "Camera");
	if (out.length === 0) out.push("Silent capture");
	return out;
}
