/**
 * `@recast/analytics` — the one analytics + error-tracking abstraction shared by
 * the web app and the desktop app.
 *
 * Call sites import ONLY from here (or `@recast/analytics/taxonomy` for the event
 * names). They never touch `posthog-js` directly, so the vendor stays swappable:
 * self-hosting is a host change, a different vendor is a new file under
 * `providers/`. See `core.ts` for the consent gate that fronts everything.
 *
 * Typical wiring (per app):
 *   const analytics = createAnalytics({
 *     provider: createPostHogBrowserProvider(),
 *     config: { apiKey, host, source, persistence, ... },
 *     initialConsent: { product, errors },
 *   });
 *   analytics.capture("app_opened");
 */

// The consent predicates and scrubbers stay unexported: `createAnalytics` applies both, and publishing them offers a way around the gate.
export { createAnalytics, type CreateAnalyticsOptions } from "./core";
export { createPostHogBrowserProvider } from "./providers/posthog-browser";
export {
	ANALYTICS_EVENTS,
	type AnalyticsEvent,
	type EventPropMap,
} from "./taxonomy";
export type {
	AnalyticsClient,
	ConsentState,
	ErrorContext,
	EventProps,
	EventSource,
	PersistenceMode,
	PropsFor,
	Provider,
	ProviderInitConfig,
	ScrubbedError,
} from "./types";
