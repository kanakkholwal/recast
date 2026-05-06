import { config } from "$constants/app";
import { LATEST_RELEASE } from "$constants/changelog";

const STORAGE_KEY = "recast-last-seen-version";

function readSeen(): string | null {
	try {
		return localStorage.getItem(STORAGE_KEY);
	} catch {
		return null;
	}
}

function writeSeen(v: string) {
	try {
		localStorage.setItem(STORAGE_KEY, v);
	} catch {
		/* localStorage unavailable — silently degrade */
	}
}

function createWhatsNewStore() {
	let open = $state(false);

	return {
		get open() {
			return open;
		},
		set open(v: boolean) {
			open = v;
		},

		// Open automatically when the app version is newer than the
		// last seen version recorded for this user. Returns true if shown.
		autoOpenIfStale(): boolean {
			const seen = readSeen();
			if (seen === config.appVersion) return false;
			open = true;
			return true;
		},

		// Open on demand without touching the seen marker. Used by manual entry points
		// (sidebar, settings, command palette) so revisiting doesn't reset state.
		show() {
			open = true;
		},

		dismiss() {
			open = false;
			writeSeen(config.appVersion);
		},

		markSeen() {
			writeSeen(config.appVersion);
		},

		latestVersion() {
			return LATEST_RELEASE.version;
		},
	};
}

export const whatsNew = createWhatsNewStore();
