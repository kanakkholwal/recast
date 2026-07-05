import { and, desc, eq, ne } from "drizzle-orm";
import { getDb } from "$lib/db";
import { folder, recast, tag } from "$lib/db/schema";
import {
	recastLatestShareSlugSql,
	recastTagIdsSql,
	recastViewsSql,
} from "$lib/db/recast-selectors";
import { resolvePlaybackUrl } from "$lib/storage";
import type { PageServerLoad } from "./$types";

/**
 * Full library loader. Larger limit than the home page.
 *
 * Returns the active recast list (each with its `folderId` + `tags` id
 * array), plus the workspace's folder tree and tag set, so the library can
 * render organization without extra client round-trips on first paint.
 *
 * Also returns the workspace's `archived` recasts — blobless rows the expiry
 * job parked after 14 days with no views. These power the "Archived" tab,
 * which lets the owner delete them outright or re-upload from desktop before
 * the hard-delete sweep purges them at `archivedAt + 16d`.
 */
export const load: PageServerLoad = async ({ parent }) => {
	const { activeOrganization } = await parent();
	const db = getDb();
	const workspaceId = activeOrganization.id;

	const [rows, folders, tags] = await Promise.all([
		db
			.select({
				id: recast.id,
				title: recast.title,
				durationSec: recast.durationSec,
				sizeBytes: recast.sizeBytes,
				source: recast.source,
				provider: recast.provider,
				status: recast.status,
				folderId: recast.folderId,
				videoUrl: recast.videoUrl,
				posterUrl: recast.posterUrl,
				createdAt: recast.createdAt,
				views: recastViewsSql(),
				latestShareSlug: recastLatestShareSlugSql(),
				// Tag id array per recast — resolved against the `tags` list
				// below in the UI. `[]` when untagged.
				tags: recastTagIdsSql(),
			})
			.from(recast)
			.where(and(eq(recast.workspaceId, workspaceId), ne(recast.status, "archived")))
			.orderBy(desc(recast.createdAt))
			.limit(200),
		db
			.select({
				id: folder.id,
				parentId: folder.parentId,
				name: folder.name,
				color: folder.color,
				path: folder.path,
			})
			.from(folder)
			.where(eq(folder.workspaceId, workspaceId)),
		db
			.select({ id: tag.id, name: tag.name, color: tag.color })
			.from(tag)
			.where(eq(tag.workspaceId, workspaceId)),
	]);

	return {
		workspaceId,
		// `videoUrl` is stored as a bare object key; sign it into a playable URL
		// the same way the share page does (signing is local/cheap per row).
		recasts: await Promise.all(
			rows.map(async (r) => ({
				...r,
				videoUrl: await resolvePlaybackUrl(r.videoUrl),
				posterUrl: await resolvePlaybackUrl(r.posterUrl),
				sizeBytes: Number(r.sizeBytes),
				views: Number(r.views ?? 0),
				createdAt: r.createdAt.getTime(),
				tags: r.tags ?? [],
			})),
		),
		folders,
		tags,
	};
};
