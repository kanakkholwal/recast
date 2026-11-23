import type { Handle, HandleServerError } from "@sveltejs/kit";
import { svelteKitHandler } from "better-auth/svelte-kit";
import { building } from "$app/environment";
import { getAuth } from "$lib/auth/server";
import { getPublicEnv } from "$lib/env/public";
import { getServerEnv } from "$lib/env/server";
import { describeError } from "$lib/server/describe-error";
import { setRequestPlatform } from "$lib/server/platform";

// Validate env at startup so the process refuses traffic with a half-configured .env; `building` skips the prerender pass.
if (!building) {
	getServerEnv();
	getPublicEnv();
}

export const handle: Handle = ({ event, resolve }) => {
	setRequestPlatform(event.platform);
	// `auth: getAuth()` is an argument, so it evaluates before svelteKitHandler's own building guard, and on a box with no DATABASE_URL that threw a 500 on every prerendered page.
	if (building) return resolve(event);
	return svelteKitHandler({ event, resolve, auth: getAuth(), building });
};

/**
 * Single funnel for unhandled/unexpected errors (errors thrown outside an
 * explicit `error(status, …)`). Expected client errors (4xx, including the
 * 404s `requireAdmin` raises) are passed through with their message; anything
 * 5xx is logged with its full stack server-side and returned to the client as
 * a generic message + correlation id, so internals never leak. `error(status)`
 * calls in handlers still produce their own response — this catches the rest.
 */
export const handleError: HandleServerError = ({ error, event, status, message }) => {
	if (status < 500) {
		return { message };
	}

	const errorId = crypto.randomUUID();
	console.error(
		`[error ${errorId}] ${event.request.method} ${event.url.pathname} → ${status} :: ${describeError(error)}`,
	);

	return { message: "Internal error", errorId };
};
