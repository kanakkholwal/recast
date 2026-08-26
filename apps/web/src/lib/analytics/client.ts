import {
	type AnalyticsClient,
	createAnalytics,
	createPostHogBrowserProvider,
} from "@recast/analytics";
import { browser } from "$app/environment";
import { getPublicEnv } from "$lib/env/public";

/**
 * The web app's analytics singleton.
 *
 * Posture (decided with the user): cookieless / memory-only by default so basic,
 * anonymous product metrics need no banner. The consent banner (see
 * `ConsentBanner.svelte`) calls `upgradePersistence()` to enable session replay +
 * a persistent profile. When `PUBLIC_POSTHOG_KEY` is blank the whole thing is a
 * no-op — same "missing config disables it" rule as `isStorageConfigured()`.
 *
 * SSR imports this module too, but `browser` is false there so `enabled` is false
 * and every call routes to the noop provider. The browser gets its own module
 * instance on hydration with the real provider.
 */

function detectOs(): string {
	if (!browser) return "unknown";
	const ua = navigator.userAgent;
	if (/Windows/i.test(ua)) return "windows";
	if (/Mac OS X|Macintosh/i.test(ua)) return "macos";
	if (/Android/i.test(ua)) return "android";
	if (/iPhone|iPad|iPod/i.test(ua)) return "ios";
	if (/Linux/i.test(ua)) return "linux";
	return "unknown";
}

const env = getPublicEnv();

export const analytics: AnalyticsClient = createAnalytics({
	provider: createPostHogBrowserProvider(),
	// Dev builds never track — only real production (`vite build`) output emits
	// analytics, so local development doesn't pollute the PostHog project.
	enabled: browser && !import.meta.env.DEV && Boolean(env.PUBLIC_POSTHOG_KEY),
	initialConsent: { product: true, errors: true },
	// Web: init on load so PostHog's automatic pageview fires immediately.
	eagerInit: true,
	config: {
		apiKey: env.PUBLIC_POSTHOG_KEY ?? "",
		host: env.PUBLIC_POSTHOG_HOST,
		source: "web",
		// Cookieless until the banner upgrades us.
		persistence: "memory",
		autocapture: true,
		capturePageview: true,
		// Replay stays off until consent; upgradePersistence() turns it on.
		disableSessionRecording: true,
		superProperties: {
			source: "web",
			app_version: __APP_VERSION__,
			os: detectOs(),
		},
	},
});
