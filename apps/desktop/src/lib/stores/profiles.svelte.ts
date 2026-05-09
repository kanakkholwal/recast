/**
 * Reactive recording-profiles store, shared across the profiles page,
 * settings page, and the recording panel.
 *
 * Pure logic (types, migration, capability signatures, device resolution)
 * lives in `$lib/profiles`. This module wraps that logic in $state so that
 * mutations in one route propagate to others without an event bus.
 */

import {
	MAX_PROFILES,
	capSig,
	ensureExactlyOneDefault,
	findDefaultProfile,
	firstFreeCombo,
	loadProfiles,
	loadProfilesEnabled,
	persistProfiles,
	persistProfilesEnabled,
	type RecordingProfile,
} from "$lib/profiles";

function createProfilesStore() {
	let profiles = $state<RecordingProfile[]>([]);
	let enabled = $state(true);
	let hydrated = $state(false);

	/** Read everything from localStorage. Idempotent — safe to call from
	 *  every onMount, only the first call does work. */
	function hydrate() {
		if (hydrated) return;
		profiles = loadProfiles();
		enabled = loadProfilesEnabled();
		// Persist once so any seeded defaults make it to disk on first launch.
		persistProfiles(profiles);
		hydrated = true;
	}

	function persist() {
		persistProfiles(profiles);
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
			persistProfilesEnabled(v);
		},

		/** Find the user's default (or first) profile. Null when list is empty. */
		default(): RecordingProfile | null {
			return findDefaultProfile(profiles);
		},

		findById(id: string): RecordingProfile | null {
			return profiles.find((p) => p.id === id) ?? null;
		},

		/** Number of unique capability combinations remaining. */
		freeSlots(): number {
			return Math.max(0, MAX_PROFILES - profiles.length);
		},

		/** First on/off combo not yet in use. Null when full. */
		nextFreeCombo() {
			return firstFreeCombo(profiles);
		},

		/** Returns the profile that already uses `next`'s capability set
		 *  (excluding `next` itself), or null. */
		duplicateOf(next: RecordingProfile): RecordingProfile | null {
			const sig = capSig(next);
			return (
				profiles.find((p) => p.id !== next.id && capSig(p) === sig) ?? null
			);
		},

		/** Insert a brand-new profile. Caller is responsible for having
		 *  validated uniqueness via `duplicateOf`. */
		insert(next: RecordingProfile) {
			const inserted = next.isDefault
				? [...profiles.map((p) => ({ ...p, isDefault: false })), next]
				: [...profiles, next];
			profiles = ensureExactlyOneDefault(inserted);
			persist();
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
			persist();
		},

		remove(id: string) {
			const victim = profiles.find((p) => p.id === id);
			if (!victim) return;
			profiles = profiles.filter((p) => p.id !== id);
			if (victim.isDefault && profiles.length > 0) {
				profiles = ensureExactlyOneDefault(profiles);
			}
			persist();
		},

		setDefault(id: string) {
			profiles = profiles.map((p) => ({ ...p, isDefault: p.id === id }));
			persist();
		},
	};
}

export const profilesStore = createProfilesStore();
