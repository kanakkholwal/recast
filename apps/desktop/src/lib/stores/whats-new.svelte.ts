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
	// The full-screen center dialog. Now only used by manual entry points
	// (sidebar, settings, command palette) — no longer auto-opened on boot.
	let open = $state(false);
	// The non-blocking bottom-right corner card shown after a version bump.
	let cardVisible = $state(false);

	return {
		get open() {
			return open;
		},
		set open(v: boolean) {
			open = v;
		},

		get cardVisible() {
			return cardVisible;
		},

		// Called once on app boot. When the running build is newer than the
		// last version the user acknowledged, surface the corner card instead
		// of interrupting them with a centered modal.
		evaluateOnBoot(): void {
			const seen = readSeen();
			if (seen === config.appVersion) return;
			cardVisible = true;
		},

		// Open the full dialog on demand without touching the seen marker, so
		// revisiting from the sidebar/command palette doesn't reset state.
		show() {
			open = true;
		},

		// Close the dialog (and card) and mark this version as seen.
		dismiss() {
			open = false;
			cardVisible = false;
			writeSeen(config.appVersion);
		},

		// Dismiss just the corner card — e.g. the user clicked through to the
		// changelog page, or hit its close button.
		dismissCard() {
			cardVisible = false;
			writeSeen(config.appVersion);
		},

		markSeen() {
			cardVisible = false;
			writeSeen(config.appVersion);
		},

		latestVersion() {
			return LATEST_RELEASE.version;
		},
	};
}

export const whatsNew = createWhatsNewStore();
