import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { organization, subscription } from "$lib/db/schema";
import { type PlanId, planOf } from "./catalog";

/**
 * Polar webhook → DB sync. `subscription` is the billing record; the
 * entitlement every gate actually reads is `organization.plan`, mirrored here
 * in the same transaction so a paid checkout grants access immediately.
 */

export type SubscriptionSync = {
	organizationId: string;
	userId: string;
	polarCustomerId: string;
	polarSubscriptionId: string;
	plan: PlanId;
	seats: number;
	status: "active" | "canceled" | "past_due" | "incomplete" | "trialing" | "unpaid";
	currentPeriodEnd: Date | null;
	cancelAtPeriodEnd: boolean;
};

/** Statuses that still grant paid entitlements. */
function grantsAccess(status: SubscriptionSync["status"]): boolean {
	return status === "active" || status === "trialing";
}

/** Upsert by organization — one subscription per workspace. */
export async function upsertSubscription(input: SubscriptionSync): Promise<void> {
	const db = getDb();
	const entitledPlan = grantsAccess(input.status) ? input.plan : "free";

	await db.transaction(async (tx) => {
		await tx
			.insert(subscription)
			.values({
				id: input.polarSubscriptionId,
				organizationId: input.organizationId,
				userId: input.userId,
				polarCustomerId: input.polarCustomerId,
				polarSubscriptionId: input.polarSubscriptionId,
				plan: input.plan,
				seats: input.seats,
				status: input.status,
				currentPeriodEnd: input.currentPeriodEnd,
				cancelAtPeriodEnd: input.cancelAtPeriodEnd,
			})
			.onConflictDoUpdate({
				target: subscription.organizationId,
				set: {
					userId: input.userId,
					polarCustomerId: input.polarCustomerId,
					polarSubscriptionId: input.polarSubscriptionId,
					plan: input.plan,
					seats: input.seats,
					status: input.status,
					currentPeriodEnd: input.currentPeriodEnd,
					cancelAtPeriodEnd: input.cancelAtPeriodEnd,
					updatedAt: new Date(),
				},
			});

		await tx
			.update(organization)
			.set({ plan: entitledPlan })
			.where(eq(organization.id, input.organizationId));
	});
}

/** Revoke paid entitlements for a workspace when its subscription ends. */
export async function downgradeToFree(organizationId: string): Promise<void> {
	const db = getDb();
	await db.transaction(async (tx) => {
		await tx
			.update(subscription)
			.set({
				plan: "free",
				status: "canceled",
				cancelAtPeriodEnd: false,
				updatedAt: new Date(),
			})
			.where(eq(subscription.organizationId, organizationId));

		await tx.update(organization).set({ plan: "free" }).where(eq(organization.id, organizationId));
	});
}

/**
 * Authoritative workspace for an existing Polar subscription. Survives intent
 * cleanup, so renewals and cancellations resolve without guessing.
 */
export async function findWorkspaceByPolarSubscription(
	polarSubscriptionId: string,
): Promise<string | null> {
	const rows = await getDb()
		.select({ organizationId: subscription.organizationId })
		.from(subscription)
		.where(eq(subscription.polarSubscriptionId, polarSubscriptionId))
		.limit(1);
	return rows[0]?.organizationId ?? null;
}

/**
 * Entitlement lookup for a workspace. Reads `organization.plan` so
 * admin-granted plans (Enterprise, comped) resolve without a Polar record.
 */
export async function getActivePlan(organizationId: string): Promise<PlanId> {
	const db = getDb();
	const rows = await db
		.select({ plan: organization.plan })
		.from(organization)
		.where(eq(organization.id, organizationId))
		.limit(1);
	return planOf(rows[0]?.plan).id;
}
