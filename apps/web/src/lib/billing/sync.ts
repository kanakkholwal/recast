import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { subscription } from "$lib/db/schema";
import type { PlanId } from "./plans";

/**
 * Webhook → DB sync. Polar is the source of truth for subscription state;
 * this mirror table is just for fast reads ("is user X on Pro?") so we
 * don't hit the Polar API on every request.
 *
 * Callers come from the `webhooks` sub-plugin in `@polar-sh/better-auth`
 * (see src/lib/auth/server.ts). Each handler receives the parsed Polar
 * event payload; map the fields you care about and forward here.
 */

export type SubscriptionSync = {
	userId: string;
	polarCustomerId: string;
	polarSubscriptionId: string;
	plan: PlanId;
	status:
		| "active"
		| "canceled"
		| "past_due"
		| "incomplete"
		| "trialing"
		| "unpaid";
	currentPeriodEnd: Date | null;
	cancelAtPeriodEnd: boolean;
};

/** Upsert by user_id — one active subscription per user. */
export async function upsertSubscription(input: SubscriptionSync): Promise<void> {
	const db = getDb();
	await db
		.insert(subscription)
		.values({
			id: input.polarSubscriptionId,
			userId: input.userId,
			polarCustomerId: input.polarCustomerId,
			polarSubscriptionId: input.polarSubscriptionId,
			plan: input.plan,
			status: input.status,
			currentPeriodEnd: input.currentPeriodEnd,
			cancelAtPeriodEnd: input.cancelAtPeriodEnd,
		})
		.onConflictDoUpdate({
			target: subscription.userId,
			set: {
				polarCustomerId: input.polarCustomerId,
				polarSubscriptionId: input.polarSubscriptionId,
				plan: input.plan,
				status: input.status,
				currentPeriodEnd: input.currentPeriodEnd,
				cancelAtPeriodEnd: input.cancelAtPeriodEnd,
				updatedAt: new Date(),
			},
		});
}

/** Mark a user back down to free when their subscription ends. */
export async function downgradeToFree(userId: string): Promise<void> {
	const db = getDb();
	await db
		.update(subscription)
		.set({
			plan: "free",
			status: "canceled",
			cancelAtPeriodEnd: false,
			updatedAt: new Date(),
		})
		.where(eq(subscription.userId, userId));
}

/** Cheap access check used by API handlers gating Pro features. */
export async function getActivePlan(userId: string): Promise<PlanId> {
	const db = getDb();
	const rows = await db
		.select({ plan: subscription.plan, status: subscription.status })
		.from(subscription)
		.where(eq(subscription.userId, userId))
		.limit(1);
	const row = rows[0];
	if (!row) return "free";
	if (row.status !== "active" && row.status !== "trialing") return "free";
	return row.plan;
}
