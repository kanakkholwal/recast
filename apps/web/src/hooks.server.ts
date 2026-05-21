import { building } from "$app/environment";
import { svelteKitHandler } from "better-auth/svelte-kit";
import { getAuth } from "$lib/auth/server";
import { getServerEnv } from "$lib/env/server";
import { getPublicEnv } from "$lib/env/public";
import type { Handle } from "@sveltejs/kit";

// Validate env at server startup. Throws synchronously if anything is missing
// or malformed so the process refuses to serve traffic with a half-configured
// .env instead of failing inside a request handler later. `building` skips this
// during the prerender pass where env isn't available.
if (!building) {
	getServerEnv();
	getPublicEnv();
}

export const handle: Handle = async ({ event, resolve }) => {
	return svelteKitHandler({ event, resolve, auth: getAuth(), building });
};
