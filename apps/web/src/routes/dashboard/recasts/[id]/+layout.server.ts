import { error } from "@sveltejs/kit";
import { and, desc, eq } from "drizzle-orm";
import { canAccessRecast } from "$lib/dashboard/access";
import { getDb } from "$lib/db";
import { recast, share } from "$lib/db/schema";
import { resolvePlaybackUrl } from "$lib/storage";
import type { LayoutServerLoad } from "./$types";

/**
 * Shared loader for the recast pages (overview + analytics). Authorizes the
 * recast against the active workspace (404 otherwise), signs its playable +
 * poster URLs, and loads its share links — everything the overview page and the
 * shared header need. The heavier analytics data (activity/engagement) is
 * loaded lazily by the analytics sub-route so the overview stays light.
 */
export const load: LayoutServerLoad = async ({ params, parent }) => {
	const { user, activeOrganization } = await parent();
	const db = getDb();
	const workspaceId = activeOrganization.id;

	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			title: recast.title,
			description: recast.description,
			durationSec: recast.durationSec,
			sizeBytes: recast.sizeBytes,
			width: recast.width,
			height: recast.height,
			source: recast.source,
			provider: recast.provider,
			status: recast.status,
			videoUrl: recast.videoUrl,
			posterUrl: recast.posterUrl,
			createdAt: recast.createdAt,
		})
		.from(recast)
		.where(and(eq(recast.id, params.id), eq(recast.workspaceId, workspaceId)))
		.limit(1);
	if (!row) error(404, "Recast not found");

	// 404 rather than 403: a member has no business learning that an id they cannot open exists.
	const allowed = canAccessRecast({
		recastOwnerId: row.ownerId,
		userId: user.id,
		workspaceRole: activeOrganization.role,
		platformRole: user.role,
	});
	if (!allowed) error(404, "Recast not found");

	const [shareRows, videoUrl, posterUrl] = await Promise.all([
		db
			.select({
				slug: share.slug,
				visibility: share.visibility,
				passwordHash: share.passwordHash,
				expiresAt: share.expiresAt,
				viewsCount: share.viewsCount,
				createdAt: share.createdAt,
			})
			.from(share)
			.where(eq(share.recastId, params.id))
			.orderBy(desc(share.createdAt)),
		resolvePlaybackUrl(row.videoUrl),
		resolvePlaybackUrl(row.posterUrl),
	]);

	return {
		recast: {
			id: row.id,
			title: row.title,
			description: row.description,
			durationSec: row.durationSec,
			sizeBytes: Number(row.sizeBytes),
			width: row.width,
			height: row.height,
			source: row.source,
			provider: row.provider,
			status: row.status,
			videoUrl,
			posterUrl,
			createdAt: row.createdAt.getTime(),
		},
		shares: shareRows.map((s) => ({
			slug: s.slug,
			visibility: s.visibility,
			viewsCount: s.viewsCount,
			hasPassword: Boolean(s.passwordHash),
			expiresAt: s.expiresAt ? s.expiresAt.getTime() : null,
			createdAt: (s.createdAt ?? new Date(0)).getTime(),
		})),
	};
};
