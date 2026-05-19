import { dev } from "$app/environment";
import { error } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types";

/**
 * The dashboard is a local-development surface only. On any deployed build
 * (`dev === false`) the entire `/dashboard` tree resolves to a 404 — auth and
 * hosting are intentionally left undecided. Locally (`vite dev`) it renders.
 */
export const ssr = false;
export const prerender = false;

export const load: LayoutLoad = () => {
	if (!dev) {
		error(404, "Not Found");
	}
	return {};
};
