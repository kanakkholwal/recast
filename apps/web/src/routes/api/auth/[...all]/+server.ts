import { getAuth } from "$lib/auth/server";
import type { RequestHandler } from "./$types";

/**
 * Catch-all for Better Auth + every plugin it mounts (Polar checkout,
 * customer portal, webhooks under `/api/auth/polar/...`).
 *
 * If auth isn't configured yet, return 503 so misrouted requests don't 500.
 */
const handler: RequestHandler = ({ request }) => {
	const auth = getAuth();
	if (!auth) {
		return new Response("Auth is not configured (set DATABASE_URL + BETTER_AUTH_SECRET).", {
			status: 503,
		});
	}
	return auth.handler(request);
};

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const PATCH = handler;
export const DELETE = handler;
export const OPTIONS = handler;
