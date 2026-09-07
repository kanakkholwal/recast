import type { Handle, HandleServerError } from "@sveltejs/kit";
import { svelteKitHandler } from "better-auth/svelte-kit";
import { building } from "$app/environment";
import { getAuth } from "$lib/auth/server";
import { getPublicEnv } from "$lib/env/public";
import { getServerEnv } from "$lib/env/server";

// Validate env at startup so the process refuses traffic with a half-configured .env; `building` skips the prerender pass.
if (!building) {
	getServerEnv();
	getPublicEnv();
}

export const handle: Handle = ({ event, resolve }) => {
	// `auth: getAuth()` is an argument, so it evaluates before svelteKitHandler's own building guard, and on a box with no DATABASE_URL that threw a 500 on every prerendered page.
	if (building) return resolve(event);
	return svelteKitHandler({ event, resolve, auth: getAuth(), building });
};

/**
 * Render any thrown value as ONE log line that can never come out empty.
 *
 * Both matter, and both were learned the hard way while chasing a prerender
 * failure that reached CI as a bare "GET /blog → 500" with no cause:
 *
 *  - Single line. Vercel's build-log capture keeps the first line of a write and
 *    drops the rest, so a newline-separated stack vanishes. Newlines are folded
 *    to " ⏎ " instead.
 *  - `||`, not `??`. An `Error` whose `stack` is the empty string (bundled and
 *    minified server output can produce one) is not nullish, so `stack ?? message`
 *    yields "" and prints nothing at all. Every field is tried in turn, and a
 *    value that still renders as nothing falls back to naming its own type.
 */
function describeError(error: unknown): string {
	const flatten = (s: string) => s.replace(/\s*\n\s*/g, " ⏎ ").trim();

	if (error instanceof Error) {
		const parts = [flatten(error.stack || "") || flatten(`${error.name}: ${error.message}`)];
		// Zod and better-auth wrap the real failure in `cause`, so without this the log stops where the detail begins.
		if (error.cause !== undefined) parts.push(`cause: ${describeError(error.cause)}`);
		// Anything the class hangs off the error beyond the standard fields, such as `issues` or `code`.
		const extras = Object.getOwnPropertyNames(error).filter(
			(k) => !["name", "message", "stack", "cause"].includes(k),
		);
		for (const key of extras) {
			parts.push(`${key}=${safeJson((error as unknown as Record<string, unknown>)[key])}`);
		}
		return parts.filter(Boolean).join(" | ");
	}

	const rendered = flatten(safeJson(error));
	return rendered && rendered !== "{}" ? rendered : `<non-error ${typeof error}: ${String(error)}>`;
}

function safeJson(value: unknown): string {
	try {
		return JSON.stringify(value) ?? String(value);
	} catch {
		return String(value);
	}
}

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
