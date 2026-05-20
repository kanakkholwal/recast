/**
 * Placeholder auth client — modelled after Better Auth's client API so the UI
 * is integration-ready. Nothing here calls a server.
 *
 * To wire Better Auth for real:
 *   1. `pnpm add better-auth` in apps/web.
 *   2. Create `src/lib/auth/server.ts`:
 *        import { betterAuth } from "better-auth";
 *        export const auth = betterAuth({ database: ..., emailAndPassword: { enabled: true }, socialProviders: { github: {...}, google: {...} } });
 *   3. Replace this file with:
 *        import { createAuthClient } from "better-auth/svelte";
 *        export const authClient = createAuthClient();
 *   4. Mount the handler at `src/routes/api/auth/[...all]/+server.ts`:
 *        import { auth } from "$lib/auth/server";
 *        import { svelteKitHandler } from "better-auth/svelte-kit";
 *        export const GET = svelteKitHandler({ auth });
 *        export const POST = svelteKitHandler({ auth });
 *   5. (Optional) protect dashboard routes via `hooks.server.ts`.
 *
 * Until then, every method here simulates success: it surfaces a toast and
 * navigates to /dashboard so the auth UI is fully clickable in local dev.
 */

import { goto } from "$app/navigation";
import { toast } from "@recast/ui/sonner";

export type AuthError = { message: string };
export type AuthResult<T = null> = { data: T | null; error: AuthError | null };

async function placeholder<T = null>(
	message: string,
	options: { redirectTo?: string | null } = {},
): Promise<AuthResult<T>> {
	const { redirectTo = "/dashboard" } = options;
	toast.info(message);
	if (redirectTo) await goto(redirectTo);
	return { data: null, error: null };
}

export type SocialProvider = "github" | "google";

export const authClient = {
	signIn: {
		email: (_input: {
			email: string;
			password: string;
			rememberMe?: boolean;
			callbackURL?: string;
		}) =>
			placeholder(
				"Auth isn't wired up — dropping you into the local dashboard.",
			),
		social: ({ provider }: { provider: SocialProvider; callbackURL?: string }) =>
			placeholder(
				`OAuth (${provider}) isn't wired up — local-dev placeholder.`,
			),
	},

	signUp: {
		email: (_input: {
			name: string;
			email: string;
			password: string;
			callbackURL?: string;
		}) =>
			placeholder("Account isn't persisted — local-dev placeholder."),
	},

	signOut: () =>
		placeholder("Signed out (local-dev placeholder).", { redirectTo: "/login" }),

	forgetPassword: (_input: { email: string; redirectTo?: string }) =>
		placeholder("Reset link sent — check your inbox (placeholder).", {
			redirectTo: null,
		}),

	resetPassword: (_input: { newPassword: string; token: string }) =>
		placeholder("Password reset — sign back in.", { redirectTo: "/login" }),
};
