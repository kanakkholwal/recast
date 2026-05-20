import { redirect } from "@sveltejs/kit";
import { getAuth } from "$lib/auth/server";
import type { LayoutServerLoad } from "./$types";

/**
 * Dashboard auth gate.
 *
 * - If Better Auth isn't configured yet (no DATABASE_URL / BETTER_AUTH_SECRET),
 *   we run in "placeholder mode" — the local-dev experience stays open so
 *   the UI and stores can be iterated on without infra. Returns `user: null`.
 *
 * - Once configured, we require a session; otherwise bounce to /login with
 *   a `next` param so the user lands back on the page they wanted.
 */
export const load: LayoutServerLoad = async ({ request, url }) => {
	const auth = getAuth();
	if (!auth) return { user: null };

	let session: { user: { id: string; name?: string | null; email: string } } | null = null;
	try {
		session = (await auth.api.getSession({ headers: request.headers })) as typeof session;
	} catch {
		// Treat DB / adapter errors as "unauthenticated" rather than crashing.
		session = null;
	}

	if (!session) {
		redirect(303, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	}

	return {
		user: {
			id: session.user.id,
			name: session.user.name ?? "",
			email: session.user.email,
		},
	};
};
