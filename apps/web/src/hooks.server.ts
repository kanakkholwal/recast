import { building } from "$app/environment";
import { svelteKitHandler } from "better-auth/svelte-kit";
import { getAuth } from "$lib/auth/server";
import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
	return svelteKitHandler({ event, resolve, auth: getAuth(), building });
};
