import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

/**
 * Sign-ups are open, so the waitlist funnel is retired. The route stays alive
 * only to catch links already out in the world (emails, the old footer, the
 * `?email=` handoff from /login) and forward them, prefill intact, to /signup.
 */
export const prerender = false;

export const load: PageLoad = ({ url }) => {
	const params = new URLSearchParams();
	const email = url.searchParams.get("email")?.trim();
	if (email) params.set("email", email);
	params.set("source", url.searchParams.get("source") ?? "waitlist");
	redirect(308, `/signup?${params}`);
};
