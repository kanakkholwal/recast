/**
 * Recording profile types, migration, and pure resolution helpers.
 *
 * The reactive store lives in `stores/profiles.svelte.ts`; this module is the
 * non-reactive core so it can be tested standalone and imported from
 * non-component code (panel page, IPC layer, etc.).
 */

import type { AudioDeviceInfo } from "$lib/ipc";
import type { BrowserCamera } from "$lib/camera/browser-devices";
import { findCamera } from "$lib/camera/browser-devices";

/** Stored profile record. v2 schema — adds device identity fields over v1. */
export interface RecordingProfile {
	id: string;
	name: string;
	systemAudio: boolean;
	microphone: boolean;
	/** Tauri/Rust audio device id; null = use system default when applied. */
	micDeviceId: string | null;
	/** Display label for the saved mic; used as fallback identity if id stale. */
	micLabel: string | null;
	camera: boolean;
	/** DirectShow-friendly name — what the Rust recorder consumes. */
	cameraLabel: string | null;
	/** Browser MediaDevices id — what the camera-preview window consumes. */
	cameraDeviceId: string | null;
	isDefault: boolean;
}

/** Pre-v2 (capability-only) shape, kept for migration. */
interface RecordingProfileV1 {
	id: string;
	name: string;
	systemAudio: boolean;
	microphone: boolean;
	camera: boolean;
	isDefault: boolean;
}

export const PROFILES_STORAGE_KEY = "recast-recording-profiles";
export const PROFILES_ENABLED_STORAGE_KEY = "recast-profiles-enabled";

/** 3 capability toggles ⇒ 2³ unique combinations. */
export const MAX_PROFILES = 8;

/** Capability fingerprint — uniqueness key for dedup. Device IDs are not
 *  part of this on purpose: two profiles with the same on/off shape but
 *  different mics is user-confusion masquerading as flexibility. */
export function capSig(
	p: Pick<RecordingProfile, "systemAudio" | "microphone" | "camera">,
): string {
	return `${+p.systemAudio}${+p.microphone}${+p.camera}`;
}

/** First on/off combo (walking 0–7) that no profile in `list` already uses,
 *  or null when all 8 are taken. */
export function firstFreeCombo(list: RecordingProfile[]): {
	systemAudio: boolean;
	microphone: boolean;
	camera: boolean;
} | null {
	const taken = new Set(list.map(capSig));
	for (let i = 0; i < 8; i++) {
		const combo = {
			systemAudio: !!(i & 4),
			microphone: !!(i & 2),
			camera: !!(i & 1),
		};
		if (!taken.has(capSig(combo))) return combo;
	}
	return null;
}

/** Enforce the "exactly one default" invariant in-place (returns a new array). */
export function ensureExactlyOneDefault(
	list: RecordingProfile[],
): RecordingProfile[] {
	if (list.length === 0) return list;
	const defaults = list.filter((p) => p.isDefault);
	if (defaults.length === 1) return list;
	if (defaults.length === 0) {
		return list.map((p, i) => (i === 0 ? { ...p, isDefault: true } : p));
	}
	let seen = false;
	return list.map((p) => {
		if (p.isDefault && !seen) {
			seen = true;
			return p;
		}
		return p.isDefault ? { ...p, isDefault: false } : p;
	});
}

/** Hand-build the seed set for first launch. Three profiles covering the
 *  most common shapes so users see the value of the system without having
 *  to click "New profile" before recording. */
export function seedProfiles(): RecordingProfile[] {
	const id = () => crypto.randomUUID();
	return [
		{
			id: id(),
			name: "Screen only",
			systemAudio: true,
			microphone: false,
			micDeviceId: null,
			micLabel: null,
			camera: false,
			cameraLabel: null,
			cameraDeviceId: null,
			isDefault: true,
		},
		{
			id: id(),
			name: "Tutorial",
			systemAudio: true,
			microphone: true,
			micDeviceId: null,
			micLabel: null,
			camera: false,
			cameraLabel: null,
			cameraDeviceId: null,
			isDefault: false,
		},
		{
			id: id(),
			name: "Presentation",
			systemAudio: true,
			microphone: true,
			micDeviceId: null,
			micLabel: null,
			camera: true,
			cameraLabel: null,
			cameraDeviceId: null,
			isDefault: false,
		},
	];
}

function isV1(p: unknown): p is RecordingProfileV1 {
	return (
		typeof p === "object" &&
		p !== null &&
		"id" in p &&
		"name" in p &&
		"systemAudio" in p &&
		"microphone" in p &&
		"camera" in p &&
		!("micDeviceId" in p)
	);
}

function isV2(p: unknown): p is RecordingProfile {
	return (
		typeof p === "object" &&
		p !== null &&
		"id" in p &&
		"micDeviceId" in p &&
		"cameraLabel" in p
	);
}

/**
 * Read profiles from localStorage. Migrates v1 rows forward (filling new
 * device fields with null). Returns `seedProfiles()` if storage is empty,
 * unparseable, or every entry was unrecognizable. Never throws.
 */
export function loadProfiles(): RecordingProfile[] {
	let raw: string | null = null;
	try {
		raw = localStorage.getItem(PROFILES_STORAGE_KEY);
	} catch {
		return seedProfiles();
	}
	if (!raw) return seedProfiles();

	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return seedProfiles();
	}
	if (!Array.isArray(parsed) || parsed.length === 0) return seedProfiles();

	const migrated: RecordingProfile[] = [];
	for (const entry of parsed) {
		if (isV2(entry)) {
			migrated.push(entry);
			continue;
		}
		if (isV1(entry)) {
			migrated.push({
				...entry,
				micDeviceId: null,
				micLabel: null,
				cameraLabel: null,
				cameraDeviceId: null,
			});
			continue;
		}
		// Drop unrecognized rows silently — better than throwing on the whole list.
	}

	if (migrated.length === 0) return seedProfiles();
	return ensureExactlyOneDefault(migrated);
}

/** Persist profiles to localStorage. Silently no-ops if storage is unavailable. */
export function persistProfiles(list: RecordingProfile[]): void {
	try {
		localStorage.setItem(PROFILES_STORAGE_KEY, JSON.stringify(list));
	} catch {
		/* unavailable — silently degrade */
	}
}

/** Read the on/off flag for the whole profile system. Defaults to enabled. */
export function loadProfilesEnabled(): boolean {
	try {
		const raw = localStorage.getItem(PROFILES_ENABLED_STORAGE_KEY);
		if (raw === null) return true;
		return raw === "true";
	} catch {
		return true;
	}
}

export function persistProfilesEnabled(enabled: boolean): void {
	try {
		localStorage.setItem(
			PROFILES_ENABLED_STORAGE_KEY,
			enabled ? "true" : "false",
		);
	} catch {
		/* unavailable — silently degrade */
	}
}

/** Pick the user's default profile, or the first one if no default flag is
 *  set. Returns null only when the list is empty. */
export function findDefaultProfile(
	list: RecordingProfile[],
): RecordingProfile | null {
	if (list.length === 0) return null;
	return list.find((p) => p.isDefault) ?? list[0];
}

// ---------- Device resolution ----------

export type DeviceResolution<T> =
	| { kind: "matched"; device: T }
	| {
		kind: "fallback";
		requestedLabel: string;
		device: T;
		reason: string;
	}
	| { kind: "missing"; requestedLabel: string }
	| { kind: "none" };

/**
 * Resolve a profile's saved mic against the currently available audio inputs.
 * Order:
 *   1. Saved deviceId still present → matched.
 *   2. Saved label matches a current device → fallback (id changed).
 *   3. System default exists → fallback (saved device gone).
 *   4. Nothing available → missing.
 *
 * Pure function — never reads from the store and never toasts. Callers
 * surface the result however the calling surface needs (panel toasts,
 * editor inline warnings, etc.).
 */
export function resolveMic(
	profile: RecordingProfile,
	available: AudioDeviceInfo[],
): DeviceResolution<AudioDeviceInfo> {
	if (!profile.microphone) return { kind: "none" };
	if (available.length === 0) {
		return {
			kind: "missing",
			requestedLabel: profile.micLabel ?? "Microphone",
		};
	}

	if (profile.micDeviceId) {
		const exact = available.find((d) => d.id === profile.micDeviceId);
		if (exact) return { kind: "matched", device: exact };
	}

	if (profile.micLabel) {
		const byLabel = available.find((d) => d.name === profile.micLabel);
		if (byLabel) {
			return {
				kind: "fallback",
				requestedLabel: profile.micLabel,
				device: byLabel,
				reason: "device id changed",
			};
		}
	}

	const def = available.find((d) => d.isDefault) ?? available[0];
	if (def && profile.micLabel) {
		return {
			kind: "fallback",
			requestedLabel: profile.micLabel,
			device: def,
			reason: "saved mic unavailable — using system default",
		};
	}
	if (def) return { kind: "matched", device: def };
	return {
		kind: "missing",
		requestedLabel: profile.micLabel ?? "Microphone",
	};
}

/**
 * Resolve a profile's saved camera against the WebView's enumerated cameras.
 * Uses the existing `findCamera` fuzzy matcher (label/id/partial), then
 * falls back to the first non-virtual cam. Same semantics as `resolveMic`.
 */
export function resolveCamera(
	profile: RecordingProfile,
	available: BrowserCamera[],
): DeviceResolution<BrowserCamera> {
	if (!profile.camera) return { kind: "none" };
	if (available.length === 0) {
		return {
			kind: "missing",
			requestedLabel: profile.cameraLabel ?? "Camera",
		};
	}

	const query = profile.cameraDeviceId ?? profile.cameraLabel;
	if (query) {
		const matched = findCamera(available, query);
		if (matched) {
			const exactId =
				profile.cameraDeviceId &&
				available.some((c) => c.deviceId === profile.cameraDeviceId);
			if (exactId) return { kind: "matched", device: matched };
			return {
				kind: "fallback",
				requestedLabel: profile.cameraLabel ?? query,
				device: matched,
				reason: "device id changed",
			};
		}
	}

	const def =
		available.find((c) => !c.isVirtual) ?? available[0] ?? null;
	if (def && (profile.cameraLabel || profile.cameraDeviceId)) {
		return {
			kind: "fallback",
			requestedLabel: profile.cameraLabel ?? profile.cameraDeviceId ?? "",
			device: def,
			reason: "saved camera unavailable — using system default",
		};
	}
	if (def) return { kind: "matched", device: def };
	return {
		kind: "missing",
		requestedLabel: profile.cameraLabel ?? "Camera",
	};
}
