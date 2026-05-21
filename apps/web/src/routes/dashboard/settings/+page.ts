import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

// /dashboard/settings has no content of its own — land on the first tab.
export const load: PageLoad = () => {
	redirect(307, "/dashboard/settings/profile");
};
