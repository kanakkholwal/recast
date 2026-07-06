import { loadRecastActivity, loadRecastEngagement } from "$lib/dashboard/activity.server";
import type { PageServerLoad } from "./$types";

/**
 * Analytics-tab data. Kept off the overview loader so opening a recast doesn't
 * pay for the activity/engagement queries. `parent()` is awaited first so the
 * layout's workspace authorization (404) resolves before we read anything.
 */
export const load: PageServerLoad = async ({ params, parent }) => {
	await parent();
	const [activity, engagement] = await Promise.all([
		loadRecastActivity(params.id),
		loadRecastEngagement(params.id),
	]);
	return { activity, engagement };
};
