/**
 * Desktop analytics singleton.
 *
 * Posture: product analytics opt-in (default OFF), crash reporting default ON.
 * `eagerInit` is omitted, so PostHog makes zero network calls until a real crash
 * or explicit opt-in. Blank `PUBLIC_POSTHOG_KEY` makes the whole client a no-op.
 * The anonymous `distinct_id` is the persistent install id, so pre-sign-in crashes
 * attribute to the same person as later identified events.
 */

import {
	type AnalyticsClient,
	createAnalytics,
	createPostHogBrowserProvider,
} from "@recast/analytics";
import { config } from "$constants/app";
import { getInstallId } from "$lib/analytics/identity";
import { POSTHOG_HOST, POSTHOG_KEY } from "$lib/env";
import { desktopConsent } from "$lib/stores/consent.svelte";

export const analytics: AnalyticsClient = createAnalytics({
	provider: createPostHogBrowserProvider(),
	// Dev builds never track, so `tauri dev` and `vite dev` don't pollute the PostHog project.
	enabled: !import.meta.env.DEV && Boolean(POSTHOG_KEY),
	initialConsent: {
		product: desktopConsent.product,
		errors: desktopConsent.errors,
	},
	config: {
		apiKey: POSTHOG_KEY ?? "",
		host: POSTHOG_HOST,
		source: "desktop",
		// localStorage, not cookies: custom-scheme webview origins don't carry cookies reliably.
		persistence: "localStorage",
		// A screen recorder must never autocapture clicks, and route pageviews are meaningless across Tauri windows.
		autocapture: false,
		capturePageview: false,
		disableSessionRecording: true,
		bootstrapDistinctId: getInstallId(),
		superProperties: {
			source: "desktop",
			app_version: config.appVersion,
		},
	},
});

/**
 * Push the consent store's current state into the analytics client. Call after
 * the user flips a toggle in Settings or accepts/declines the first-run prompt.
 */
export function syncConsent() {
	analytics.setConsent({
		product: desktopConsent.product,
		errors: desktopConsent.errors,
	});
}
