import { polarClient } from "@polar-sh/better-auth";
import {
	adminClient,
	deviceAuthorizationClient,
	magicLinkClient,
	organizationClient,
} from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/svelte";
import { browser } from "$app/environment";
import { publicEnv } from "$lib/env/public";

/**
 * Better Auth client. Backed by /api/auth/* (mounted by
 * src/routes/api/auth/[...all]/+server.ts, configured in
 * src/lib/auth/server.ts).
 *
 * Methods we use:
 *   authClient.signIn.email({ email, password, rememberMe })
 *   authClient.signIn.magicLink({ email, callbackURL })
 *   authClient.signIn.social({ provider, callbackURL })   // dev only
 *   authClient.signOut()
 *   authClient.requestPasswordReset({ email, redirectTo })
 *   authClient.resetPassword({ newPassword, token })
 *
 * Polar (billing) adds:
 *   authClient.checkout({ slug: "pro" })
 *   authClient.customer.portal()
 *
 * Reactive session: `authClient.useSession()` returns a Svelte store with
 * `data` / `isPending` / `error`.
 */
/**
 * Always resolved here, never left for Better Auth to guess.
 *
 * Given no `baseURL`, `getBaseURL()` falls back to scanning env — BETTER_AUTH_URL,
 * NEXT_PUBLIC_BETTER_AUTH_URL, PUBLIC_BETTER_AUTH_URL, NUXT_*, BASE_URL — and
 * runs the first hit through `new URL()`, throwing if it has no scheme. This
 * module is imported by the root layout, so that throw happens at module scope
 * on *every* page, which during prerendering meant a 500 on every page under
 * /blog and /tools. Note it bypasses `serverEnv()` entirely, so the env schema
 * never gets a chance to reject the bad value first.
 *
 * The client only ever calls its own origin's /api/auth, so:
 *  - browser: the live origin, which keeps preview deployments talking to
 *    themselves instead of to production.
 *  - server (SSR/prerender): PUBLIC_APP_URL, which the schema has already
 *    validated as a URL and defaults to a valid one when unset.
 */
const baseURL = browser ? window.location.origin : publicEnv().PUBLIC_APP_URL;

export const authClient = createAuthClient({
	baseURL,
	plugins: [
		magicLinkClient(),
		adminClient(),
		organizationClient(),
		deviceAuthorizationClient(),
		polarClient(),
	],
});

/** Providers we expose social buttons for (dev only). */
export type SocialProvider = "github" | "google";
