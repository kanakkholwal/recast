/**
 * Recording profile types, migration, and pure resolution helpers.
 *
 * The reactive store lives in `stores/profiles.svelte.ts`; this module is the
 * non-reactive core so it can be tested standalone and imported from
 * non-component code (panel page, IPC layer, etc.).
 */

import { safeStorage } from "@recast/ui/persisted-state";
import type { AudioDeviceInfo } from "./wire-types";
import type { BrowserCamera } from "./camera/browser-devices";
import { findCamera } from "./camera/browser-devices";

/** Stored profile record. v2 schema, adding device identity fields over v1. */
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
	/** DirectShow-friendly name: what the Rust recorder consumes. */
	cameraLabel: string | null;
	/** Browser MediaDevices id: what the camera-preview window consumes. */
	cameraDeviceId: string | null;
	/**
	 * Per-profile countdown override (seconds). `null`/absent = inherit the
	 * global countdown; `0` = off. Part of `capSig`, so two profiles can share
	 * devices but differ by pre-roll. Option space: `COUNTDOWN_OPTIONS`.
	 */
	countdown?: number | null;
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

// Global capture quality + frame rate. Capture-WIDE preferences (like the
// global countdown), so they live outside profile records, kept clear of
// `capSig` and the combination cap, applied to every recording.

export const RECORDING_QUALITY_STORAGE_KEY = "recast-recording-quality";
export const RECORDING_FPS_STORAGE_KEY = "recast-recording-fps";

/** Capture quality tier. `"auto"` lets the backend pick against the detected
 *  encoder (hardware → high, software → balanced). Tiers mirror the Rust
 *  `RecordingQuality` enum. */
export type RecordingQuality = "auto" | "balanced" | "high" | "pristine";

/** Read the persisted capture quality tier. Unrecognized values → "auto". */
export function loadRecordingQuality(): RecordingQuality {
	const v = safeStorage.get<string>(RECORDING_QUALITY_STORAGE_KEY, "auto");
	return v === "balanced" || v === "high" || v === "pristine" ? v : "auto";
}

export function persistRecordingQuality(q: RecordingQuality): void {
	safeStorage.set(RECORDING_QUALITY_STORAGE_KEY, q);
}

/** Read the persisted capture frame rate. `null` = "Auto" (backend default
 *  60). Out-of-range values are coerced back to `null`. */
export function loadRecordingFps(): number | null {
	const v = safeStorage.get<number | null>(RECORDING_FPS_STORAGE_KEY, null);
	return typeof v === "number" && v >= 24 && v <= 240 ? Math.round(v) : null;
}

export function persistRecordingFps(fps: number | null): void {
	safeStorage.set(RECORDING_FPS_STORAGE_KEY, fps);
}

// Slot sentinels, distinct from any specific device id and from each other.
const DEFAULT_SLOT = "default";
const OFF_SLOT = "off";

/**
 * Countdown override option space: single source of truth for the editor UI,
 * `capSig`, and the combination cap. `null` = inherit global; `0` = off; rest
 * pin an explicit pre-roll. Keep `null` first so the auto-pick walk exhausts
 * every device combo at "inherit" before introducing pinned countdowns.
 */
export const COUNTDOWN_OPTIONS: readonly (number | null)[] = [null, 0, 3, 5, 10];

/** Stable slot token for a countdown value, used in `capSig` and combo walks.
 *  `null`/absent → "inherit"; otherwise the literal seconds. */
export function countdownToken(cd: number | null | undefined): string {
	return cd == null ? "inherit" : String(cd);
}

function micSlot(p: Pick<RecordingProfile, "microphone" | "micDeviceId">): string {
	if (!p.microphone) return OFF_SLOT;
	return p.micDeviceId ?? DEFAULT_SLOT;
}
function camSlot(p: Pick<RecordingProfile, "camera" | "cameraDeviceId">): string {
	if (!p.camera) return OFF_SLOT;
	return p.cameraDeviceId ?? DEFAULT_SLOT;
}

/**
 * Capability fingerprint: dedup key, **including** device identity (same
 * on/off shape with different mic/cam ids are intentionally distinct presets).
 * Slots: `off`, `default` (runtime picks system default), or a literal device
 * id; trailing segment is the countdown slot. See `COUNTDOWN_OPTIONS`.
 */
export function capSig(p: RecordingProfile): string {
	return `${+p.systemAudio}|${micSlot(p)}|${camSlot(p)}|${countdownToken(p.countdown)}`;
}

/** Enforce the "exactly one default" invariant in-place (returns a new array). */
export function ensureExactlyOneDefault(list: RecordingProfile[]): RecordingProfile[] {
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

/** Seed set for first launch: three profiles covering the common shapes. */
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
		typeof p === "object" && p !== null && "id" in p && "micDeviceId" in p && "cameraLabel" in p
	);
}

/**
 * Read profiles from localStorage. Migrates v1 rows forward (filling new
 * device fields with null). Returns `seedProfiles()` if storage is empty,
 * unparseable, or every entry was unrecognizable. Never throws.
 */
export function loadProfiles(): RecordingProfile[] {
	// `safeStorage` returns [] for missing key / no-window / malformed JSON,
	// so every empty-ish case funnels into the seed below.
	const parsed = safeStorage.get<unknown[]>(PROFILES_STORAGE_KEY, []);
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
		// Drop unrecognized rows rather than throwing on the whole list.
	}

	if (migrated.length === 0) return seedProfiles();
	return ensureExactlyOneDefault(migrated);
}

/** Read the on/off flag for the whole profile system. Defaults to enabled. */
export function loadProfilesEnabled(): boolean {
	return safeStorage.get<boolean>(PROFILES_ENABLED_STORAGE_KEY, true);
}

/**
 * Read the pre-backend `localStorage` profiles, or `null` when the key was never
 * written. The backend is now the store; this exists only so the store can
 * migrate an existing user's saved profiles into it once, then delete the key.
 * A present-but-empty key still returns a (migrated) list so `enabled` carries
 * over. Distinguished from `loadProfiles`, which seeds instead of returning null.
 */
export function readLegacyProfiles(): { profiles: RecordingProfile[]; enabled: boolean } | null {
	// `null` sentinel: absent key -> null; present key -> the parsed value.
	const raw = safeStorage.get<unknown[] | null>(PROFILES_STORAGE_KEY, null);
	if (raw === null) return null;
	return { profiles: loadProfiles(), enabled: loadProfilesEnabled() };
}

/** Delete the legacy `localStorage` profile keys once migrated to the backend. */
export function clearLegacyProfileStorage(): void {
	safeStorage.remove(PROFILES_STORAGE_KEY);
	safeStorage.remove(PROFILES_ENABLED_STORAGE_KEY);
}

/**
 * Resolve the profile set on hydrate, given the backend snapshot and this
 * client's legacy `localStorage` read (`null` if it never had one). Returns the
 * set to show plus whether to persist it back to the backend:
 *   - backend already persisted (`initialized`) -> adopt it, no push (steady state).
 *   - backend only seeded + legacy present -> migrate the user's saved profiles up.
 *   - backend only seeded + no legacy (fresh install) -> persist the backend seed.
 * Pure so it can be unit-tested.
 */
export function reconcileProfileHydration(
	backend: { profiles: RecordingProfile[]; enabled: boolean; initialized: boolean },
	legacy: { profiles: RecordingProfile[]; enabled: boolean } | null,
): { profiles: RecordingProfile[]; enabled: boolean; push: boolean } {
	if (backend.initialized) {
		return {
			profiles: ensureExactlyOneDefault(backend.profiles),
			enabled: backend.enabled,
			push: false,
		};
	}
	if (legacy) {
		return {
			profiles: ensureExactlyOneDefault(legacy.profiles),
			enabled: legacy.enabled,
			push: true,
		};
	}
	return {
		profiles: ensureExactlyOneDefault(backend.profiles),
		enabled: backend.enabled,
		push: true,
	};
}

/** The default profile, or the first one if no default flag is set; null only
 *  when the list is empty. */
export function findDefaultProfile(list: RecordingProfile[]): RecordingProfile | null {
	if (list.length === 0) return null;
	return list.find((p) => p.isDefault) ?? list[0];
}

// Device resolution

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
 * Pure: never reads the store or toasts; callers surface the result.
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
			reason: "saved mic unavailable, using system default",
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
				profile.cameraDeviceId && available.some((c) => c.deviceId === profile.cameraDeviceId);
			if (exactId) return { kind: "matched", device: matched };
			return {
				kind: "fallback",
				requestedLabel: profile.cameraLabel ?? query,
				device: matched,
				reason: "device id changed",
			};
		}
	}

	const def = available.find((c) => !c.isVirtual) ?? available[0] ?? null;
	if (def && (profile.cameraLabel || profile.cameraDeviceId)) {
		return {
			kind: "fallback",
			requestedLabel: profile.cameraLabel ?? profile.cameraDeviceId ?? "",
			device: def,
			reason: "saved camera unavailable, using system default",
		};
	}
	if (def) return { kind: "matched", device: def };
	return {
		kind: "missing",
		requestedLabel: profile.cameraLabel ?? "Camera",
	};
}
