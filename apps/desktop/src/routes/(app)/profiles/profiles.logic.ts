/**
 * Pure profile-dialog helpers: the card sub-line summary, dialog sizing, draft
 * construction for new/duplicated profiles, and the pre-save device-pointer
 * normalization. No store, DOM, or IPC: callers pass probed devices in.
 */

import type { RecordingProfile } from "$lib/profiles";

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
export function computeDialogWidth(viewportWidth: number, showDevicePanel: boolean): number {
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
		parts.push(profile.countdown === 0 ? "No countdown" : `${profile.countdown}s countdown`);
	}
	return parts.length === 0 ? "Screen capture only" : parts.join(" · ");
}

/**
 * A saveable draft for a brand-new profile: screen plus system audio, no mic or
 * camera, inheriting the global countdown. The user tunes it from there.
 */
export function buildNewDraft(profileCount: number): RecordingProfile {
	return {
		id: crypto.randomUUID(),
		name: `Profile ${profileCount + 1}`,
		systemAudio: true,
		microphone: false,
		micDeviceId: null,
		micLabel: null,
		camera: false,
		cameraDeviceId: null,
		cameraLabel: null,
		countdown: null,
		isDefault: profileCount === 0,
	};
}

/** A non-default copy of `profile` under a fresh id, opened as an editable draft. */
export function buildDuplicate(profile: RecordingProfile): RecordingProfile {
	return {
		...profile,
		id: crypto.randomUUID(),
		name: `${profile.name} Copy`,
		isDefault: false,
	};
}

/**
 * True when the draft differs from the profile the dialog was opened with, so a
 * stray Escape or click-outside only prompts when there is something to lose.
 * Compared post-normalization: clearing a device pointer that a disabled
 * capability was going to drop anyway is not a change worth warning about.
 */
export function isDraftDirty(draft: RecordingProfile, original: RecordingProfile): boolean {
	const a = normalizeProfileForSave({ ...draft, name: draft.name.trim() });
	const b = normalizeProfileForSave({ ...original, name: original.name.trim() });
	return (
		a.name !== b.name ||
		a.systemAudio !== b.systemAudio ||
		a.microphone !== b.microphone ||
		a.micDeviceId !== b.micDeviceId ||
		a.camera !== b.camera ||
		a.cameraDeviceId !== b.cameraDeviceId ||
		(a.countdown ?? null) !== (b.countdown ?? null) ||
		a.isDefault !== b.isDefault
	);
}

/**
 * Another profile sharing this one's name, ignoring case and surrounding space.
 * The picker and the CLI both identify a profile by name, so two called
 * "Meeting" are indistinguishable in every list that shows them.
 */
export function nameClashOf(
	draft: RecordingProfile,
	profiles: RecordingProfile[],
): RecordingProfile | null {
	const name = draft.name.trim().toLowerCase();
	if (!name) return null;
	return profiles.find((p) => p.id !== draft.id && p.name.trim().toLowerCase() === name) ?? null;
}

/**
 * When a capability is off, clear the matching device pointers so we don't
 * persist stale identity that won't be applied anyway.
 */
export function normalizeProfileForSave(profile: RecordingProfile): RecordingProfile {
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
