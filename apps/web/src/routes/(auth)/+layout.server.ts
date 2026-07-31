import { redirect } from "@sveltejs/kit";
import { enabledSocialProviders, getAuth } from "$lib/auth/server";
import { safeNext } from "$lib/auth/redirect";
import type { LayoutServerLoad } from "./$types";

/**
 * Routes that mean nothing to a signed-in user — landing on them with a live
 * session is a dead end, so bounce to `?next=` (or the dashboard).
 *
 * The rest of the group is deliberately absent: /verify-email, /device and
 * /accept-invitation all *require* a session, and /reset-password has to stay
 * reachable so someone can finish a reset in a browser they're still signed
 * into.
 */
const SIGNED_IN_DEAD_ENDS = new Set(["/login", "/signup", "/forgot-password", "/waitlist"]);

export const load: LayoutServerLoad = async ({ request, url }) => {
	const socialProviders = enabledSocialProviders();
	if (!SIGNED_IN_DEAD_ENDS.has(url.pathname)) return { socialProviders };

	const session = await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null);
	if (session) redirect(303, safeNext(url.searchParams.get("next")));

	return { socialProviders };
};
