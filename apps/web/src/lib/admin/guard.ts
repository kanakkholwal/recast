import { error, redirect, type RequestEvent } from "@sveltejs/kit";
import { getAuth } from "$lib/auth/server";

/**
 * Server-side gate for `/admin/*` loads + actions. Returns the session if the
 * caller is an admin; otherwise:
 *   - unauthenticated → redirect to /login?next=…
 *   - authenticated, not admin → 404 (don't disclose that /admin exists).
 *
 * Better Auth's admin plugin already 403's its own /admin/* API endpoints
 * regardless of UI gating — this is purely so non-admins never see the page.
 *
 * Bootstrap the first admin manually:
 *   UPDATE "user" SET role='admin' WHERE email='you@example.com';
 */
export type AdminSession = {
	user: {
		id: string;
		email: string;
		name: string;
		image: string | null;
		role: string;
		status: string;
	};
	session: {
		id: string;
		token: string;
		impersonatedBy: string | null;
	};
};

export async function requireAdmin(event: RequestEvent): Promise<AdminSession> {
	const raw = (await getAuth()
		.api.getSession({ headers: event.request.headers })
		.catch(() => null)) as AdminSession | null;

	if (!raw) {
		const next = encodeURIComponent(event.url.pathname + event.url.search);
		redirect(303, `/login?next=${next}`);
	}
	if (raw.user.role !== "admin") {
		error(404, "Not found");
	}
	return raw;
}
