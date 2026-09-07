import { error, json } from "@sveltejs/kit";
import { eq, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { recast, workspaceUsage } from "$lib/db/schema";
import { authorizeRecast } from "$lib/server/recast-guard";
import { deleteRecastObjects } from "$lib/storage/recast-objects";
import type { RequestHandler } from "./$types";

/**
 * POST /api/recasts/[id]/archive
 *
 * Manually archive a published recast to reclaim storage: flip
 * `status='archived'`, zero its size, reclaim `storage_bytes`, move the usage
 * counters (active → archived), then drop the video blob. Mirrors the
 * auto-expiry archive step so quota accounting stays consistent. The row
 * (analytics + metadata) is kept and the cron hard-deletes it later; the video
 * is gone, so this is not reversible without re-uploading.
 *
 * Creator, workspace owner/admin, or platform admin. Idempotent: archiving an
 * already-archived recast is a no-op.
 */
export const POST: RequestHandler = async ({ params, request }) => {
	const row = await authorizeRecast(request, params.id);

	if (row.status === "archived") return json({ ok: true }); // idempotent
	if (row.status !== "published") error(409, "Only published recasts can be archived");

	// State first, object after: a failed transaction must not leave a published row whose video is gone. Served delivery is never reversed.
	await getDb().transaction(async (tx) => {
		await tx
			.update(recast)
			.set({ status: "archived", archivedAt: new Date(), sizeBytes: 0, updatedAt: new Date() })
			.where(eq(recast.id, row.id));
		await tx
			.update(workspaceUsage)
			.set({
				storageBytes: sql`GREATEST(${workspaceUsage.storageBytes} - ${row.sizeBytes}, 0)`,
				activeRecastsCount: sql`GREATEST(${workspaceUsage.activeRecastsCount} - 1, 0)`,
				archivedRecastsCount: sql`${workspaceUsage.archivedRecastsCount} + 1`,
				updatedAt: new Date(),
			})
			.where(eq(workspaceUsage.workspaceId, row.workspaceId));
	});

	// The poster stays: the archive list renders it, and a thumbnail isn't what the storage bill is made of.
	await deleteRecastObjects(row.id, [row.videoUrl]);

	return json({ ok: true });
};
