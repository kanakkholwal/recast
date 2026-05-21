import { redirect } from "@sveltejs/kit";
import { getAuth } from "$lib/auth/server";
import type { LayoutServerLoad } from "./$types";

type SessionUser = { id: string; name?: string | null; email: string; role?: string | null };
type SessionShape = { user: SessionUser };

/**
 * Dashboard auth gate. Require a session; otherwise bounce to /login with a
 * `next` param so the user lands back on the page they wanted.
 */
export const load: LayoutServerLoad = async ({ request, url }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session) {
		redirect(303, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	}

	return {
		user: {
			id: session.user.id,
			name: session.user.name ?? "",
			email: session.user.email,
			role: session.user.role ?? "user",
		},
	};
};
