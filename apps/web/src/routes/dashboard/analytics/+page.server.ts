import { and, desc, eq, ne } from "drizzle-orm";
import { loadWorkspaceActivity, loadWorkspacePerformance } from "$lib/dashboard/activity.server";
import { getDb } from "$lib/db";
import { recastViewsSql } from "$lib/db/recast-selectors";
import { recast } from "$lib/db/schema";
import { resolvePlaybackUrl } from "$lib/storage";
import type { PageServerLoad } from "./$types";

/**
 * Analytics loader. Pulls real viewer events from `share_view` (via
 * `loadWorkspaceActivity`) and the per-recast performance rollups behind the
 * comparison table. The page derives every range, delta and breakdown from
 * `activity` on the client, so changing the range costs no round-trip.
 */
export const load: PageServerLoad = async ({ parent }) => {
	const { activeOrganization } = await parent();
	const db = getDb();
	const workspaceId = activeOrganization.id;

	const [recasts, activity, perf] = await Promise.all([
		db
			.select({
				id: recast.id,
				title: recast.title,
				posterUrl: recast.posterUrl,
				views: recastViewsSql(),
			})
			.from(recast)
			.where(and(eq(recast.workspaceId, workspaceId), ne(recast.status, "archived")))
			.orderBy(desc(recast.createdAt))
			.limit(200),
		loadWorkspaceActivity(workspaceId),
		loadWorkspacePerformance(workspaceId),
	]);

	// Sign each poster key once, reused by both the performance table thumbnails and the recasts list.
	const posterFor = new Map<string, string>(
		await Promise.all(
			recasts.map(async (r) => [r.id, await resolvePlaybackUrl(r.posterUrl)] as const),
		),
	);

	// Views use the cached share total; the rest of the comparison rows come from the aggregate rollups.
	const performance = recasts.map((r) => {
		const p = perf.get(r.id);
		return {
			id: r.id,
			title: r.title,
			posterUrl: posterFor.get(r.id) ?? "",
			views: Number(r.views ?? 0),
			avgWatch: p?.avgWatch ?? 0,
			completion: p?.completion ?? 0,
			comments: p?.comments ?? 0,
		};
	});
	// Only activity and performance reach the page; the full recast list and a lifetime comment total used to ship unread.
	return { activity, performance };
};
