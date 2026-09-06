/**
 * Desktop telemetry consent, persisted to localStorage AND mirrored into the
 * Rust `AppConfig` so the native crash reporter can read the `errors` flag.
 *
 * Defaults:
 *   - `product` (behaviour analytics): OFF, strictly opt-in.
 *   - `errors`  (crash reporting):     ON, default opt-in, toggle to disable.
 *
 * `PersistedState` gives cross-window `storage` sync (Tauri v2 webviews share a
 * localStorage origin, so a toggle in Settings reaches open editor windows
 * without a reload); the Rust mirror is layered on in the setters.
 */

import { PersistedState, safeStorage } from "@recast/ui/persisted-state";
import { getInstallId } from "$lib/analytics/identity";
import { setTelemetryConsent } from "$lib/ipc";

export interface DesktopConsent {
	product: boolean;
	errors: boolean;
}

const DEFAULTS: DesktopConsent = { product: false, errors: true };

const STORAGE_KEY = "recast-telemetry-consent";
const SEEN_KEY = "recast-consent-seen";

/**
 * Mirror consent into Rust so the panic hook can read `errors` and attribute
 * crashes to the same install id as JS events. Called only on explicit toggles
 * (cross-window re-reads were already mirrored by the originating window).
 */
function mirrorToRust(consent: DesktopConsent) {
	void setTelemetryConsent(consent.product, consent.errors, getInstallId()).catch(() => {
		// Non-Tauri preview: JS-side gating still applies.
	});
}

function createConsentStore() {
	// Merged over DEFAULTS, so a consent key added in a later build keeps its default for existing users.
	const consent = new PersistedState<DesktopConsent>(STORAGE_KEY, DEFAULTS);

	return {
		get product() {
			return consent.current.product;
		},
		get errors() {
			return consent.current.errors;
		},
		/** Has the first-run privacy moment been shown + dismissed yet? */
		get hasSeenFirstRun() {
			// SSR/no-window: treat as seen so the prompt never flashes pre-hydration.
			if (typeof window === "undefined") return true;
			return safeStorage.get<string>(SEEN_KEY, "") === "1";
		},
		markFirstRunSeen() {
			safeStorage.set(SEEN_KEY, "1");
		},
		setProduct(value: boolean) {
			consent.current = { ...consent.current, product: value };
			mirrorToRust(consent.current);
		},
		setErrors(value: boolean) {
			consent.current = { ...consent.current, errors: value };
			mirrorToRust(consent.current);
		},
	};
}

export const desktopConsent = createConsentStore();
