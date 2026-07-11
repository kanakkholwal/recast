/**
 * Reactive recording-profiles store, shared across the profiles/settings pages
 * and the recording panel. Pure logic lives in `$lib/profiles`; this wraps it
 * in $state so mutations in one route propagate to others without an event bus.
 *
 * The backend (`commands/profiles.rs`) is the sole store, so the CLI (`recast
 * profile list/use`) reads and writes the same set. `localStorage` is no longer
 * a persistence layer: `hydrate()` loads from the backend, migrates a pre-backend
 * `localStorage` set up once, then deletes the legacy key. Edits push to the
 * backend, which broadcasts `recording-profiles:changed` to every window. The
 * change listener only assigns state (never re-pushes), so the sync loop is
 * structurally broken regardless of the ordering guard.
 */

import { listen } from "@tauri-apps/api/event";
import {
	getProfiles,
	RECORDING_PROFILES_CHANGED_EVENT,
	setProfiles,
	type ProfilesSnapshot,
} from "$lib/ipc";
import {
	capSig,
	clearLegacyProfileStorage,
	ensureExactlyOneDefault,
	findDefaultProfile,
	readLegacyProfiles,
	reconcileProfileHydration,
	type RecordingProfile,
} from "$lib/profiles";

/** Stable-ish signature of a profile set, for the best-effort echo guard. */
function signature(profiles: RecordingProfile[], enabled: boolean): string {
	return JSON.stringify({ enabled, profiles });
}

function createProfilesStore() {
	let profiles = $state<RecordingProfile[]>([]);
	let enabled = $state(true);
	let hydrated = $state(false);
	// Signature of the set we last pushed to (or adopted from) the backend. When
	// our own `set_profiles` echoes back as `recording-profiles:changed`, this
	// lets us skip the redundant re-assign. Best-effort only; loop-safety does
	// not depend on it (the listener never pushes).
	let lastSynced = "";
	// One-shot: hydrate is idempotent across the many onMount call sites.
	let hydratePromise: Promise<void> | null = null;

	function setState(next: RecordingProfile[], nextEnabled: boolean) {
		profiles = next;
		enabled = nextEnabled;
		lastSynced = signature(profiles, enabled);
	}

	/** Push the current set to the backend. Fire-and-forget: the UI already
	 *  reflects it optimistically, and a failed persist is logged, not surfaced. */
	function pushToBackend() {
		lastSynced = signature(profiles, enabled);
		setProfiles($state.snapshot(profiles), enabled).catch((e) => {
			console.warn("failed to persist profiles to backend", e);
		});
	}

	async function doHydrate() {
		if (typeof window !== "undefined") {
			// Cross-window sync rides the backend broadcast (Tauri `emit` reaches
			// every webview). The listener only assigns; it never pushes back.
			listen<ProfilesSnapshot>(RECORDING_PROFILES_CHANGED_EVENT, (event) => {
				const snap = event.payload;
				if (signature(snap.profiles, snap.enabled) === lastSynced) return;
				setState(ensureExactlyOneDefault(snap.profiles), snap.enabled);
			}).catch(() => {});
		}

		try {
			const snap = await getProfiles();
			const legacy = readLegacyProfiles();
			const { profiles: next, enabled: nextEnabled, push } =
				reconcileProfileHydration(snap, legacy);
			setState(next, nextEnabled);
			if (push) pushToBackend();
			// The backend is now authoritative; drop the pre-backend key so it
			// can't linger or drift.
			clearLegacyProfileStorage();
		} catch {
			// Backend unreachable (e.g. a non-Tauri context): fall back to the
			// legacy read so the UI still shows something. Leave the key in place.
			const legacy = readLegacyProfiles();
			if (legacy) setState(ensureExactlyOneDefault(legacy.profiles), legacy.enabled);
		} finally {
			hydrated = true;
		}
	}

	/** Load the profile set from the backend (idempotent). Resolves once the set
	 *  is populated, so callers that need it immediately (panel default-profile,
	 *  picker highlight) can await it. */
	function hydrate(): Promise<void> {
		return (hydratePromise ??= doHydrate());
	}

	return {
		hydrate,

		get profiles() {
			return profiles;
		},
		get enabled() {
			return enabled;
		},
		get hydrated() {
			return hydrated;
		},

		setEnabled(v: boolean) {
			enabled = v;
			pushToBackend();
		},

		/** Find the user's default (or first) profile. Null when list is empty. */
		default(): RecordingProfile | null {
			return findDefaultProfile(profiles);
		},

		findById(id: string): RecordingProfile | null {
			return profiles.find((p) => p.id === id) ?? null;
		},

		/** Another profile with the same capture settings (excluding `next`
		 *  itself), or null. Used for a soft "you already have one like this"
		 *  nudge, not to block saving. */
		twinOf(next: RecordingProfile): RecordingProfile | null {
			const sig = capSig(next);
			return (
				profiles.find((p) => p.id !== next.id && capSig(p) === sig) ?? null
			);
		},

		/** Insert a brand-new profile. */
		insert(next: RecordingProfile) {
			const inserted = next.isDefault
				? [...profiles.map((p) => ({ ...p, isDefault: false })), next]
				: [...profiles, next];
			profiles = ensureExactlyOneDefault(inserted);
			pushToBackend();
		},

		/** Update an existing profile in place. */
		update(next: RecordingProfile) {
			if (next.isDefault) {
				profiles = profiles.map((p) => ({
					...(p.id === next.id ? next : p),
					isDefault: p.id === next.id,
				}));
			} else {
				profiles = profiles.map((p) => (p.id === next.id ? next : p));
				profiles = ensureExactlyOneDefault(profiles);
			}
			pushToBackend();
		},

		remove(id: string) {
			const victim = profiles.find((p) => p.id === id);
			if (!victim) return;
			profiles = profiles.filter((p) => p.id !== id);
			if (victim.isDefault && profiles.length > 0) {
				profiles = ensureExactlyOneDefault(profiles);
			}
			pushToBackend();
		},

		setDefault(id: string) {
			profiles = profiles.map((p) => ({ ...p, isDefault: p.id === id }));
			pushToBackend();
		},
	};
}

export const profilesStore = createProfilesStore();
