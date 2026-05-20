import { dev } from "$app/environment";
import { error } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types";

/**
 * Auth pages are local-development only — they exist so the sign-up / sign-in
 * UI can be designed and demoed before Better Auth lands. Any deployed build
 * (`dev === false`) 404s the entire (auth) tree.
 */
export const ssr = false;
export const prerender = false;

export const load: LayoutLoad = () => {
	if (!dev) {
		error(404, "Not Found");
	}
	return {};
};
