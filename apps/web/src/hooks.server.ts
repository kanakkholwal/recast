import { svelteKitHandler } from "better-auth/svelte-kit";
import { getAuth } from "$lib/auth/server";
import type { Handle } from "@sveltejs/kit";
import { building } from "$app/environment";

/**
 * Mount the Better Auth handler when env is configured; pass through cleanly
 * otherwise so `pnpm dev` keeps working before the database is wired.
 */
export const handle: Handle = async ({ event, resolve }) => {
	const auth = getAuth();
	if (!auth) return resolve(event);
	return svelteKitHandler({ event, resolve, auth, building });
};
