import { error, json } from "@sveltejs/kit";
import { eq, sql } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { recast, user, workspaceUsage } from "$lib/db/schema";
import { deleteObject } from "$lib/storage";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string } };

/**
 * POST /api/recasts/[id]/archive
 *
 * Manually archive a published recast to reclaim storage: delete the video
 * blob, flip `status='archived'`, zero its size, reclaim `storage_bytes`, and
 * move the usage counters (active → archived). Mirrors the auto-expiry archive
 * step so quota accounting stays consistent — the row (analytics + metadata) is
 * kept and the cron hard-deletes it later. The video itself is gone, so this is
 * not reversible without re-uploading. Owner or global admin only. Idempotent:
 * archiving an already-archived recast is a no-op.
 */
export const POST: RequestHandler = async ({ params, request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	const db = getDb();
	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
			sizeBytes: recast.sizeBytes,
			status: recast.status,
		})
		.from(recast)
		.where(eq(recast.id, params.id))
		.limit(1);
	if (!row) error(404, "Recast not found");

	const isOwner = row.ownerId === session.user.id;
	if (!isOwner) {
		const [u] = await db
			.select({ role: user.role })
			.from(user)
			.where(eq(user.id, session.user.id))
			.limit(1);
		if (u?.role !== "admin") error(403, "Not allowed to archive this recast");
	}

	if (row.status === "archived") return json({ ok: true }); // idempotent
	if (row.status !== "published") error(409, "Only published recasts can be archived");

	// Best-effort blob delete — bare keys only (skip legacy absolute URLs). A
	// provider 404 is fine; we still flip the row so accounting is reclaimed.
	if (row.videoUrl && !/^https?:\/\//.test(row.videoUrl)) {
		await deleteObject(row.videoUrl).catch((err) => {
			console.error(`[recasts/archive] blob delete failed for ${row.id} — row still archived`, err);
		});
	}

	await db.transaction(async (tx) => {
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

	return json({ ok: true });
};
