import { polarClient } from "@polar-sh/better-auth";
import { magicLinkClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/svelte";

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
export const authClient = createAuthClient({
	plugins: [magicLinkClient(), polarClient()],
});

/** Providers we expose social buttons for (dev only). */
export type SocialProvider = "github" | "google";
