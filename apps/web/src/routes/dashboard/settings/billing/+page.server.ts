import { eq } from "drizzle-orm";
import { polarProductIdFor, PLANS } from "$lib/billing/plans";
import { getDb } from "$lib/db";
import { subscription as subscriptionTable } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
	const { user } = await parent();
	const [subscription] = await getDb()
		.select({
			plan: subscriptionTable.plan,
			status: subscriptionTable.status,
			currentPeriodEnd: subscriptionTable.currentPeriodEnd,
			cancelAtPeriodEnd: subscriptionTable.cancelAtPeriodEnd,
			polarCustomerId: subscriptionTable.polarCustomerId,
		})
		.from(subscriptionTable)
		.where(eq(subscriptionTable.userId, user.id))
		.limit(1);

	return {
		subscription: subscription ?? null,
		billingConfigured: Boolean(polarProductIdFor("pro")),
		plans: Object.values(PLANS).map((plan) => ({
			id: plan.id,
			name: plan.name,
			priceUsd: plan.priceUsd,
			limits: {
				activeShares: Number.isFinite(plan.limits.activeShares)
					? plan.limits.activeShares
					: null,
				analytics: plan.limits.analytics,
				customBranding: plan.limits.customBranding,
				passwordProtection: plan.limits.passwordProtection,
				linkExpiry: plan.limits.linkExpiry,
				watermark: plan.limits.watermark,
			},
		})),
	};
};
