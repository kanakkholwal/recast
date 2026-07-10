/**
 * Pure profile-dialog helpers: the card sub-line summary, dialog sizing, draft
 * construction for new/duplicated profiles, and the pre-save device-pointer
 * normalization. No store, DOM, or IPC: callers pass probed devices in.
 */

import type { BrowserCamera } from "$lib/camera/browser-devices";
import type { AudioDeviceInfo } from "$lib/ipc";
import type { ProfileCombo, RecordingProfile } from "$lib/profiles";

// Device pickers live in a side panel so the dialog grows wider, not taller,
// when a capability is enabled. Below `sm` they fall back inline.
export const DIALOG_MAIN_W = 408;
export const DIALOG_ASIDE_W = 300;

/** Below this the dialog is single-column and device pickers render inline. */
export function isCompactViewport(viewportWidth: number): boolean {
	return viewportWidth < 640;
}

/**
 * Compact = fluid up to a cap; wide = fixed form column, plus the aside width
 * when the device panel is showing.
 */
export function computeDialogWidth(
	viewportWidth: number,
	showDevicePanel: boolean,
): number {
	if (isCompactViewport(viewportWidth)) {
		return Math.min(440, viewportWidth - 32);
	}
	return showDevicePanel ? DIALOG_MAIN_W + DIALOG_ASIDE_W : DIALOG_MAIN_W;
}

/**
 * Header sub-line. The faceplate already shows WHICH sources are on, so this
 * carries only the specifics: device names + an explicit countdown override.
 */
export function summarize(profile: RecordingProfile): string {
	const parts: string[] = [];
	if (profile.microphone) parts.push(profile.micLabel ?? "Default mic");
	if (profile.camera) parts.push(profile.cameraLabel ?? "Camera");
	if (profile.countdown != null) {
		parts.push(
			profile.countdown === 0
				? "No countdown"
				: `${profile.countdown}s countdown`,
		);
	}
	return parts.length === 0 ? "Screen capture only" : parts.join(" · ");
}

/**
 * A saveable draft for the next free capability combination. Resolves device
 * labels for any specific id the combo picked so the dropdown opens pre-filled
 * and the saved profile carries an identity.
 */
export function buildDraftFromCombo(
	combo: ProfileCombo,
	mics: AudioDeviceInfo[],
	cameras: BrowserCamera[],
	profileCount: number,
): RecordingProfile {
	const micDevice = combo.micDeviceId
		? mics.find((m) => m.id === combo.micDeviceId)
		: null;
	const camDevice = combo.cameraDeviceId
		? cameras.find((c) => c.deviceId === combo.cameraDeviceId)
		: null;
	return {
		id: crypto.randomUUID(),
		name: `Profile ${profileCount + 1}`,
		systemAudio: combo.systemAudio,
		microphone: combo.microphone,
		micDeviceId: combo.micDeviceId,
		micLabel: micDevice?.name ?? null,
		camera: combo.camera,
		cameraDeviceId: combo.cameraDeviceId,
		cameraLabel: camDevice?.label ?? null,
		// Carry the auto-picked countdown so the profile lands on the free combo
		// instead of serializing as "inherit" and colliding.
		countdown: combo.countdown,
		isDefault: profileCount === 0,
	};
}

/**
 * A non-default copy of `profile` under a fresh id. Opened as a draft: the
 * user must change a capability before Save (the duplicate-signature check
 * would otherwise reject it).
 */
export function buildDuplicate(profile: RecordingProfile): RecordingProfile {
	return {
		...profile,
		id: crypto.randomUUID(),
		name: `${profile.name} Copy`,
		isDefault: false,
	};
}

/**
 * When a capability is off, clear the matching device pointers so we don't
 * persist stale identity that won't be applied anyway.
 */
export function normalizeProfileForSave(
	profile: RecordingProfile,
): RecordingProfile {
	const next = { ...profile };
	if (!next.microphone) {
		next.micDeviceId = null;
		next.micLabel = null;
	}
	if (!next.camera) {
		next.cameraLabel = null;
		next.cameraDeviceId = null;
	}
	return next;
}
