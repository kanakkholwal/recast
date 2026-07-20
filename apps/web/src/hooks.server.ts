import { building } from "$app/environment";
import { svelteKitHandler } from "better-auth/svelte-kit";
import { getAuth } from "$lib/auth/server";
import { getServerEnv } from "$lib/env/server";
import { getPublicEnv } from "$lib/env/public";
import type { Handle, HandleServerError } from "@sveltejs/kit";

// Validate env at server startup. Throws synchronously if anything is missing
// or malformed so the process refuses to serve traffic with a half-configured
// .env instead of failing inside a request handler later. `building` skips this
// during the prerender pass where env isn't available.
if (!building) {
	getServerEnv();
	getPublicEnv();
}

export const handle: Handle = async ({ event, resolve }) => {
	// `svelteKitHandler` already no-ops while building, but `auth: getAuth()` is
	// an argument, so it is evaluated *before* that guard runs — and building the
	// instance validates the server env and opens the Drizzle/pg adapter. The
	// prerenderer runs hooks for every prerendered page, so on a build box with
	// no DATABASE_URL / BETTER_AUTH_SECRET that throw turned into a 500 on every
	// page under /blog and /tools. Nothing prerendered ever hits /api/auth, so
	// return before touching auth at all.
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
	// One string, not two console.error args: build/deploy log pipelines line-wrap
	// and drop trailing args, which is how a prerender failure reached CI as a
	// bare "GET /blog → 500" with the cause missing.
	const detail = error instanceof Error ? (error.stack ?? error.message) : String(error);
	console.error(
		`[error ${errorId}] ${event.request.method} ${event.url.pathname} → ${status}\n${detail}`,
	);

	return { message: "Internal error", errorId };
};
