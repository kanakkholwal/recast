import { redirect } from "@sveltejs/kit";
import { APIError } from "better-auth/api";
import { getAuth } from "$lib/auth/server";
import type { PageServerLoad } from "./$types";

type SessionShape = { user: { id: string; email: string; name?: string | null } };

/**
 * Verification entry point for the OAuth 2.0 Device Authorization Grant
 * (RFC 8628). The desktop app opens the user's default browser here at
 * `/device?user_code=XXXX-XXXX` (built from the plugin's
 * `verification_uri_complete`).
 *
 * Flow per Better Auth's docs:
 *
 *   • No user_code → render the manual code-entry form. Sign-in not
 *     required to view this page; we'll redirect to /login *after* the
 *     user submits a code.
 *   • user_code present, not signed in → REDIRECT to /login with a return
 *     URL. The docs are explicit: "Users must be authenticated when
 *     calling GET /device, because the verification step binds the
 *     pending device code to that session. Only the same session can
 *     later approve or deny." Calling GET /device unauthenticated would
 *     fail to bind the row, so the eventual approve call would either
 *     reject or — worse — bind to whichever session signs in next. Sending
 *     them through /login first guarantees the binding lands on the right
 *     account.
 *   • user_code present, signed in → call the plugin's GET /device endpoint
 *     server-side (passing the session headers so it binds the row to the
 *     viewer's userId) and hand the verified record to the page.
 */
export const load: PageServerLoad = async ({ url, request }) => {
	const userCode = url.searchParams.get("user_code")?.trim() ?? null;

	const auth = getAuth();
	const session = (await auth.api
		.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	// Entry-form variant — no code yet, sign-in not required to render.
	if (!userCode) {
		return {
			userCode: null,
			device: null,
			viewer: session ? { email: session.user.email, name: session.user.name ?? null } : null,
			error: null,
		};
	}

	// Code present but no session → bounce through login with a return URL.
	// The /login page reads ?next= and re-routes after sign-in (see
	// (auth)/login/+page.svelte). When the user lands back here their
	// session cookie is set and the deviceVerify call below will bind the
	// deviceCode row to them.
	if (!session) {
		const returnTo = `/device?user_code=${encodeURIComponent(userCode)}`;
		throw redirect(303, `/login?next=${encodeURIComponent(returnTo)}`);
	}

	// Authenticated path — verify + bind in one call.
	try {
		const device = await auth.api.deviceVerify({
			query: { user_code: userCode },
			headers: request.headers,
		});
		return {
			userCode,
			device,
			viewer: { email: session.user.email, name: session.user.name ?? null },
			error: null,
		};
	} catch (err) {
		const message =
			err instanceof APIError
				? ((err.body as { error_description?: string })?.error_description ??
					"Invalid or expired code.")
				: "Invalid or expired code.";
		return {
			userCode,
			device: null,
			viewer: { email: session.user.email, name: session.user.name ?? null },
			error: message,
		};
	}
};
