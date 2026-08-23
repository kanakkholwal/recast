import { desc, eq } from "drizzle-orm";
import { loadWorkspaceActivity } from "$lib/dashboard/activity.server";
import { getDb } from "$lib/db";
import { recastLatestShareSlugSql, recastViewsSql } from "$lib/db/recast-selectors";
import { recast } from "$lib/db/schema";
import { resolvePlaybackUrl } from "$lib/storage";
import type { PageServerLoad } from "./$types";

/**
 * Dashboard home loader. The layout above has already resolved the
 * active workspace and quota — here we just fetch the most recent
 * non-archived recasts for the metrics cards, activity feed, and
 * "Top recasts" rail.
 *
 * Trimmed to 12 — enough to fill all three rails on the home page;
 * the full library lives at /dashboard/recasts.
 */
export const load: PageServerLoad = async ({ parent }) => {
	const { activeOrganization } = await parent();
	const db = getDb();

	const [recasts, activity] = await Promise.all([
		db
			.select({
				id: recast.id,
				title: recast.title,
				durationSec: recast.durationSec,
				sizeBytes: recast.sizeBytes,
				source: recast.source,
				provider: recast.provider,
				status: recast.status,
				videoUrl: recast.videoUrl,
				posterUrl: recast.posterUrl,
				createdAt: recast.createdAt,
				views: recastViewsSql(),
				latestShareSlug: recastLatestShareSlugSql(),
			})
			.from(recast)
			.where(eq(recast.workspaceId, activeOrganization.id))
			.orderBy(desc(recast.createdAt))
			.limit(12),
		loadWorkspaceActivity(activeOrganization.id),
	]);

	return {
		// Surfaced so the home page can upload into the active workspace
		// (mirrors what the library loader returns).
		workspaceId: activeOrganization.id,
		// `videoUrl` is a bare object key — sign it into a playable URL (mirrors
		// the share page; signing is local, and the list is capped at 12 here).
		recasts: await Promise.all(
			recasts
				.filter((r) => r.status !== "archived")
				.map(async (r) => ({
					...r,
					videoUrl: await resolvePlaybackUrl(r.videoUrl),
					posterUrl: await resolvePlaybackUrl(r.posterUrl),
					sizeBytes: Number(r.sizeBytes),
					views: Number(r.views ?? 0),
					createdAt: r.createdAt.getTime(),
				})),
		),
		activity,
	};
};
