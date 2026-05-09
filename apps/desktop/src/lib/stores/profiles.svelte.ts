/**
 * Reactive recording-profiles store, shared across the profiles page,
 * settings page, and the recording panel.
 *
 * Pure logic (types, migration, capability signatures, device resolution)
 * lives in `$lib/profiles`. This module wraps that logic in $state so that
 * mutations in one route propagate to others without an event bus.
 */

import {
	capSig,
	ensureExactlyOneDefault,
	findDefaultProfile,
	firstFreeCombo,
	loadProfiles,
	loadProfilesEnabled,
	maxCombinations,
	persistProfiles,
	persistProfilesEnabled,
	type ProfileCombo,
	type RecordingProfile,
} from "$lib/profiles";

interface DeviceLite {
	id: string;
	name: string;
}
interface CameraLite {
	deviceId: string;
	label: string;
}

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

		/** Theoretical max profiles given currently-attached devices. */
		maxCombinations(mics: DeviceLite[], cams: CameraLite[]): number {
			return maxCombinations(mics.length, cams.length);
		},

		/** Slots in the device-aware cartesian that are not yet used by any
		 *  profile. Returns 0 when every attainable combo is taken. */
		freeSlots(mics: DeviceLite[], cams: CameraLite[]): number {
			const max = maxCombinations(mics.length, cams.length);
			const taken = new Set(profiles.map(capSig));
			// Count attainable signatures that aren't yet taken. We can't just
			// subtract `profiles.length` because saved profiles may reference
			// devices that are no longer attached — those don't consume a slot
			// in the *current* cartesian.
			let count = 0;
			const micOpts = ["off", "default", ...mics.map((m) => m.id)];
			const camOpts = ["off", "default", ...cams.map((c) => c.deviceId)];
			for (const sa of [1, 0]) {
				for (const mic of micOpts) {
					for (const cam of camOpts) {
						if (!taken.has(`${sa}|${mic}|${cam}`)) count++;
					}
				}
			}
			// Bound to [0, max] in case `taken` happens to overlap nothing.
			return Math.min(max, Math.max(0, count));
		},

		/** First combo not yet in use against the current device cartesian. */
		nextFreeCombo(
			mics: DeviceLite[],
			cams: CameraLite[],
		): ProfileCombo | null {
			return firstFreeCombo(profiles, mics, cams);
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
