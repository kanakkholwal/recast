import { safeStorage } from "@recast/ui/persisted-state";
import { config } from "$constants/app";
import { LATEST_RELEASE } from "$constants/changelog";

const STORAGE_KEY = "recast-last-seen-version";

// Raw version string (not JSON); the "" fallback also doubles as "unset".
function readSeen(): string {
	return safeStorage.get<string>(STORAGE_KEY, "");
}

function writeSeen(v: string) {
	safeStorage.set(STORAGE_KEY, v);
}

function createWhatsNewStore() {
	// Full-screen dialog, manual entry points only (no longer auto-opened on boot).
	let open = $state(false);
	// Non-blocking corner card shown after a version bump.
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

		// On boot, surface the corner card (not a modal) when the build is newer than the last acknowledged version.
		evaluateOnBoot(): void {
			const seen = readSeen();
			if (seen === config.appVersion) return;
			cardVisible = true;
		},

		// Open on demand without touching the seen marker.
		show() {
			open = true;
		},

		dismiss() {
			open = false;
			cardVisible = false;
			writeSeen(config.appVersion);
		},

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
