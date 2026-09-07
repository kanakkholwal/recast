import type { PostHog } from "posthog-js";
import type { Provider, ProviderInitConfig, ScrubbedError } from "../types";

/**
 * PostHog provider, backed by `posthog-js`. The ONLY file in the codebase that
 * imports `posthog-js` — everything else goes through the `Provider` interface,
 * so swapping vendors or self-hosting is contained here.
 *
 * Works in both the web app and the Tauri desktop webview; the differences
 * (persistence, autocapture, pageviews, session recording, bootstrap id) all
 * arrive through `ProviderInitConfig`, which each app's client builds.
 *
 * `posthog-js` is dynamically imported inside `init` so it stays out of the SSR
 * bundle and is only fetched once consent has actually stood the provider up.
 */
export function createPostHogBrowserProvider(): Provider {
	let ph: PostHog | null = null;
	let config: ProviderInitConfig | null = null;

	return {
		async init(cfg) {
			if (ph) return;
			config = cfg;
			const mod = await import("posthog-js");
			const posthog = mod.default;
			posthog.init(cfg.apiKey, {
				api_host: cfg.host,
				persistence: cfg.persistence,
				autocapture: cfg.autocapture,
				// `history_change` covers the initial load and SPA navigations; desktop passes false and emits explicit events instead.
				capture_pageview: cfg.capturePageview ? "history_change" : false,
				capture_pageleave: cfg.capturePageview,
				disable_session_recording: cfg.disableSessionRecording,
				// Only create a person profile once a user is identified, so anonymous visitors mint no person rows.
				person_profiles: "identified_only",
				bootstrap: cfg.bootstrapDistinctId ? { distinctID: cfg.bootstrapDistinctId } : undefined,
			});
			if (cfg.superProperties) posthog.register(cfg.superProperties);
			ph = posthog;
		},

		capture(event, props) {
			ph?.capture(event, props);
		},

		identify(userId, traits) {
			ph?.identify(userId, traits);
		},

		reset() {
			ph?.reset();
		},

		captureError(err: ScrubbedError) {
			// Error Tracking ingests `$exception`; the payload is pre-scrubbed and the stack goes in as text, with no client-side frame parser.
			ph?.capture("$exception", {
				$exception_list: [
					{
						type: err.name,
						value: err.message,
						stacktrace: err.stack ? { type: "raw", frames: [], raw: err.stack } : undefined,
					},
				],
				$exception_type: err.name,
				$exception_message: err.message,
				error_fingerprint: err.fingerprint,
				...err.context,
			});
		},

		register(props) {
			ph?.register(props);
		},

		optIn() {
			ph?.opt_in_capturing();
		},

		optOut() {
			ph?.opt_out_capturing();
		},

		upgradePersistence() {
			if (!config) return;
			// Mutate the stored config so an in-flight `init()` stands PostHog up already upgraded; this is only ever called to enable replay.
			config.persistence = "localStorage+cookie";
			config.disableSessionRecording = false;
			if (!ph) return; // a pending init() will apply the upgraded config
			ph.set_config({ persistence: "localStorage+cookie" });
			ph.startSessionRecording();
		},

		isFeatureEnabled(flag) {
			return ph?.isFeatureEnabled(flag);
		},

		shutdown() {
			ph?.opt_out_capturing();
			ph?.reset();
		},
	};
}
