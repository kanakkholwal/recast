import { and, eq, isNull, lt, or, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { organization } from "$lib/db/schema/organization";
import { recast } from "$lib/db/schema/recasts";
import { QUOTA, workspaceUsage } from "$lib/db/schema/usage";
import { deleteObject } from "$lib/storage";

/**
 * Expiry sweep — runs from a cron. Two passes per call:
 *
 *   1. **Archive sweep** — finds published cloud recasts on Free
 *      workspaces with no views in `QUOTA.free.expireAfterNoViewsDays`
 *      days. Deletes the R2 blob, flips `status='archived'`, stamps
 *      `archivedAt`, decrements active / increments archived counters
 *      in `workspace_usage`, and reclaims `storage_bytes`.
 *
 *   2. **Hard-delete sweep** — finds rows in `archived` state older
 *      than `QUOTA.free.hardDeleteAfterArchiveDays` days. Drops the
 *      row entirely; share rows cascade-delete via FK.
 *
 * Pro and Enterprise workspaces are never touched — their QUOTA entries
 * have `expireAfterNoViewsDays: null`.
 *
 * The function is **idempotent on partial failure**: a row whose R2
 * delete throws is left untouched (we don't mark it archived until the
 * blob is actually gone), so a retry picks it up.
 */
export async function runExpirySweep(): Promise<{
	archived: number;
	hardDeleted: number;
	r2Failures: number;
}> {
	const archived = await archiveStaleFreeRecasts();
	const hardDeleted = await hardDeleteArchivedFreeRecasts();
	return {
		archived: archived.count,
		hardDeleted: hardDeleted.count,
		r2Failures: archived.r2Failures + hardDeleted.r2Failures,
	};
}

async function archiveStaleFreeRecasts(): Promise<{
	count: number;
	r2Failures: number;
}> {
	const days = QUOTA.free.expireAfterNoViewsDays;
	if (days == null) return { count: 0, r2Failures: 0 };

	const cutoff = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
	const db = getDb();

	// Free-plan workspaces only; Pro and Enterprise fall through, matching their null `expireAfterNoViewsDays`.
	const candidates = await db
		.select({
			id: recast.id,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
			sizeBytes: recast.sizeBytes,
		})
		.from(recast)
		.innerJoin(organization, eq(recast.workspaceId, organization.id))
		.where(
			and(
				eq(recast.status, "published"),
				eq(recast.source, "cloud"),
				eq(organization.plan, "free"),
				or(
					and(isNull(recast.lastViewedAt), lt(recast.createdAt, cutoff)),
					lt(recast.lastViewedAt, cutoff),
				),
			),
		);

	let r2Failures = 0;
	let count = 0;

	for (const row of candidates) {
		try {
			await deleteObject(row.videoUrl);
		} catch (err) {
			r2Failures++;
			console.error(`[expire] R2 delete failed for ${row.id}`, err);
			continue;
		}

		await db.transaction(async (tx) => {
			await tx
				.update(recast)
				.set({
					status: "archived",
					archivedAt: new Date(),
					sizeBytes: 0,
					updatedAt: new Date(),
				})
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

		count++;
	}

	return { count, r2Failures };
}

async function hardDeleteArchivedFreeRecasts(): Promise<{
	count: number;
	r2Failures: number;
}> {
	const days = QUOTA.free.hardDeleteAfterArchiveDays;
	if (days == null) return { count: 0, r2Failures: 0 };

	const cutoff = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
	const db = getDb();

	const candidates = await db
		.select({
			id: recast.id,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
		})
		.from(recast)
		.innerJoin(organization, eq(recast.workspaceId, organization.id))
		.where(
			and(
				eq(recast.status, "archived"),
				eq(organization.plan, "free"),
				lt(recast.archivedAt, cutoff),
			),
		);

	let r2Failures = 0;
	let count = 0;

	for (const row of candidates) {
		// Best-effort delete: the blob is nominally gone after archiving, but a half-successful archive could leave it. 404 is success.
		try {
			await deleteObject(row.videoUrl);
		} catch (err) {
			r2Failures++;
			console.error(`[expire] R2 hard-delete failed for ${row.id}`, err);
			// Continue with the row delete: orphan blobs are reclaimed by a lifecycle rule rather than blocking cleanup forever.
		}

		await db.transaction(async (tx) => {
			await tx.delete(recast).where(eq(recast.id, row.id));
			await tx
				.update(workspaceUsage)
				.set({
					archivedRecastsCount: sql`GREATEST(${workspaceUsage.archivedRecastsCount} - 1, 0)`,
					updatedAt: new Date(),
				})
				.where(eq(workspaceUsage.workspaceId, row.workspaceId));
		});

		count++;
	}

	return { count, r2Failures };
}
