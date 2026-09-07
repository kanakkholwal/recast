import { eq, sql } from "drizzle-orm";
import { limitsFor, planOf } from "$lib/billing/catalog";
import { getDb } from "$lib/db";
import { organization } from "$lib/db/schema/organization";
import { workspaceUsage } from "$lib/db/schema/usage";
import { currentDeliveryPeriodStart, type PlanKey, type QuotaSnapshot } from "./quota.logic";

export * from "./quota.logic";

// Drizzle's transaction yields a tx-bound instance with the same query surface but a different type, so strip `$client`.
type DbLike = Omit<ReturnType<typeof getDb>, "$client">;

/**
 * Single-trip read of plan + usage. Returns `null` if the workspace doesn't
 * exist — caller decides whether that's a 404 or auto-init.
 */
export async function getQuotaSnapshot(workspaceId: string): Promise<QuotaSnapshot | null> {
	const db = getDb();

	const [org] = await db
		.select({
			plan: organization.plan,
			seatLimit: organization.seatLimit,
			storageLimitBytes: organization.storageLimitBytes,
			deliveryLimitBytes: organization.deliveryLimitBytes,
			activeRecastsLimit: organization.activeRecastsLimit,
		})
		.from(organization)
		.where(eq(organization.id, workspaceId))
		.limit(1);
	if (!org) return null;

	const [usage] = await db
		.select()
		.from(workspaceUsage)
		.where(eq(workspaceUsage.workspaceId, workspaceId))
		.limit(1);

	const plan: PlanKey = planOf(org.plan).id;

	return {
		plan,
		// Contract overrides win over the plan template; both are concrete.
		limits: limitsFor(plan, org),
		usage: {
			storageBytes: usage?.storageBytes ?? 0,
			activeRecastsCount: usage?.activeRecastsCount ?? 0,
			archivedRecastsCount: usage?.archivedRecastsCount ?? 0,
			membersCount: usage?.membersCount ?? 1,
			deliveryBytesThisMonth: usage?.deliveryBytesThisMonth ?? 0,
			deliveryPeriodStart: usage?.deliveryPeriodStart ?? currentDeliveryPeriodStart(),
		},
	};
}

/**
 * Bump usage counters after a successful upload. Endpoints SHOULD run the
 * recast UPDATE and this UPSERT in a single `db.transaction`, or a retry
 * double-counts.
 */
export async function bumpUsageOnUpload(
	workspaceId: string,
	sizeBytes: number,
	tx?: DbLike,
): Promise<void> {
	const db = tx ?? getDb();
	await db
		.insert(workspaceUsage)
		.values({
			workspaceId,
			storageBytes: sizeBytes,
			activeRecastsCount: 1,
		})
		.onConflictDoUpdate({
			target: workspaceUsage.workspaceId,
			set: {
				storageBytes: sql`${workspaceUsage.storageBytes} + ${sizeBytes}`,
				activeRecastsCount: sql`${workspaceUsage.activeRecastsCount} + 1`,
				updatedAt: new Date(),
			},
		});
}

/**
 * Reverse the bump when a recast is deleted (not archived — archive zeroes
 * `sizeBytes` and decrements active, but keeps the archived counter).
 */
export async function decrementUsageOnDelete(
	workspaceId: string,
	sizeBytes: number,
	tx?: DbLike,
): Promise<void> {
	const db = tx ?? getDb();
	await db
		.update(workspaceUsage)
		.set({
			// GREATEST guards against drift sending the counter negative.
			storageBytes: sql`GREATEST(${workspaceUsage.storageBytes} - ${sizeBytes}, 0)`,
			activeRecastsCount: sql`GREATEST(${workspaceUsage.activeRecastsCount} - 1, 0)`,
			updatedAt: new Date(),
		})
		.where(eq(workspaceUsage.workspaceId, workspaceId));
}

/**
 * Add bytes served to the current month, rolling the window over atomically so
 * concurrent views can't both reset it and lose each other's counts.
 */
export async function recordDelivery(
	workspaceId: string,
	bytes: number,
	tx?: DbLike,
	now = new Date(),
): Promise<void> {
	if (bytes <= 0) return;
	const db = tx ?? getDb();
	const periodStart = currentDeliveryPeriodStart(now);
	await db
		.insert(workspaceUsage)
		.values({
			workspaceId,
			deliveryBytesThisMonth: bytes,
			deliveryPeriodStart: periodStart,
		})
		.onConflictDoUpdate({
			target: workspaceUsage.workspaceId,
			set: {
				deliveryBytesThisMonth: sql`CASE WHEN ${workspaceUsage.deliveryPeriodStart} < ${periodStart} THEN ${bytes} ELSE ${workspaceUsage.deliveryBytesThisMonth} + ${bytes} END`,
				deliveryPeriodStart: sql`GREATEST(${workspaceUsage.deliveryPeriodStart}, ${periodStart})`,
				updatedAt: new Date(),
			},
		});
}
