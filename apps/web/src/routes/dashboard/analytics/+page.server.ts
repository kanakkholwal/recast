import { and, desc, eq, ne } from "drizzle-orm";
import { getDb } from "$lib/db";
import { recast } from "$lib/db/schema";
import { loadWorkspaceActivity, loadWorkspacePerformance } from "$lib/dashboard/activity.server";
import { recastViewsSql } from "$lib/db/recast-selectors";
import { resolvePlaybackUrl } from "$lib/storage";
import type { PageServerLoad } from "./$types";

/**
 * Analytics loader. Pulls real viewer events from `share_view` (via
 * `loadWorkspaceActivity`), the workspace's recasts with their cached view
 * totals, and the per-recast performance rollups that drive the comparison
 * table — all reflecting actual engagement.
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
				durationSec: recast.durationSec,
				sizeBytes: recast.sizeBytes,
				source: recast.source,
				provider: recast.provider,
				createdAt: recast.createdAt,
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

	// Sign each recast's poster key into a displayable URL once (cheap/local per
	// row), reused by both the performance table thumbnails and the recasts list.
	const posterFor = new Map<string, string>(
		await Promise.all(
			recasts.map(async (r) => [r.id, await resolvePlaybackUrl(r.posterUrl)] as const),
		),
	);

	// Per-recast comparison rows (views/avg watch/completion/comments). Views use
	// the cached share total; the rest come from the aggregate rollups.
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
	const commentsTotal = performance.reduce((s, p) => s + p.comments, 0);

	return {
		recasts: recasts.map((r) => ({
			id: r.id,
			title: r.title,
			durationSec: r.durationSec,
			sizeBytes: Number(r.sizeBytes),
			source: r.source,
			provider: r.provider,
			views: Number(r.views ?? 0),
			createdAt: r.createdAt.getTime(),
			posterUrl: posterFor.get(r.id) ?? "",
		})),
		activity,
		performance,
		commentsTotal,
	};
};
