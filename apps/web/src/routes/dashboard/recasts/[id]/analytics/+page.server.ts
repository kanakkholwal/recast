import { planOf } from "$lib/billing/plans";
import {
	loadRecastActivity,
	loadRecastBasicStats,
	loadRecastEngagement,
} from "$lib/dashboard/activity.server";
import type { PageServerLoad } from "./$types";

/**
 * Analytics-tab data. Kept off the overview loader so opening a recast doesn't
 * pay for the activity/engagement queries. `parent()` is awaited first so the
 * layout's workspace authorization (404) resolves before we read anything.
 *
 * Free workspaces get two aggregates and nothing else: the gated dimensions
 * (country, device, referrer, watch curve, comments, reactions) are never
 * queried, so a blurred panel is a real lock rather than a CSS filter.
 */
export const load: PageServerLoad = async ({ params, parent }) => {
	const { quota, activeOrganization } = await parent();
	const plan = planOf(quota?.plan ?? activeOrganization.plan);

	if (!plan.features.analytics) {
		return {
			analyticsUnlocked: false as const,
			basic: await loadRecastBasicStats(params.id),
			activity: [],
			engagement: null,
		};
	}

	const [activity, engagement] = await Promise.all([
		loadRecastActivity(params.id),
		loadRecastEngagement(params.id),
	]);
	return { analyticsUnlocked: true as const, basic: null, activity, engagement };
};
