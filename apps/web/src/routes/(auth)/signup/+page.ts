import { dev } from "$app/environment";
import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

/**
 * Sign-up is locked down in production — Recast Cloud is invite-only while
 * the first wave onboards. Send people to /waitlist instead. The dev server
 * still serves the form so we can keep iterating on the UI.
 */
export const load: PageLoad = () => {
	if (!dev) redirect(307, "/waitlist?source=signup");
};
