import { polarClient } from "@polar-sh/better-auth";
import { createAuthClient } from "better-auth/svelte";

/**
 * Better Auth client. Backed by the server at /api/auth/* (mounted by
 * src/routes/api/auth/[...all]/+server.ts, configured in src/lib/auth/server.ts).
 *
 * The Polar client plugin adds `authClient.checkout({ slug: "pro" })` and
 * `authClient.customer.portal()` for upgrade flows.
 *
 * Every call returns `{ data, error }` — callers handle navigation/toasts
 * themselves. Examples:
 *
 *   const { error } = await authClient.signIn.email({ email, password });
 *   if (error) toast.error(error.message);
 *   else goto("/dashboard");
 *
 *   await authClient.signOut();   // session cleared — navigate manually
 *
 * Reactive session: `authClient.useSession()` returns a Svelte store with
 * `data` / `isPending` / `error` for components that need to know who's
 * signed in.
 */
export const authClient = createAuthClient({
	plugins: [polarClient()],
});

/** Providers we expose social buttons for. Add to `authClient` provider list
 *  in `src/lib/auth/server.ts` when expanding. */
export type SocialProvider = "github" | "google";
