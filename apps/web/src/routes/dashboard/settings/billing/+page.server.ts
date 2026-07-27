import { eq } from "drizzle-orm";
import {
	DELIVERY_OVERAGE_USD_PER_GB,
	PLANS,
	planOf,
	polarProductIdFor,
	priceForSeats,
} from "$lib/billing/plans";
import { getDb } from "$lib/db";
import { subscription as subscriptionTable } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
	const { user, activeOrganization, quota } = await parent();

	// Billing is workspace-scoped: the subscription is looked up by workspace,
	// never by user. A user can own several workspaces on different plans.
	const [subscription] = await getDb()
		.select({
			plan: subscriptionTable.plan,
			status: subscriptionTable.status,
			seats: subscriptionTable.seats,
			currentPeriodEnd: subscriptionTable.currentPeriodEnd,
			cancelAtPeriodEnd: subscriptionTable.cancelAtPeriodEnd,
			polarCustomerId: subscriptionTable.polarCustomerId,
			userId: subscriptionTable.userId,
		})
		.from(subscriptionTable)
		.where(eq(subscriptionTable.organizationId, activeOrganization.id))
		.limit(1);

	const plan = planOf(quota?.plan ?? activeOrganization.plan);
	const seats = subscription?.seats ?? quota?.usage.membersCount ?? 1;

	return {
		plan: {
			id: plan.id,
			name: plan.name,
			monthlyUsd: plan.monthlyUsd,
			annualMonthlyUsd: plan.annualMonthlyUsd,
			seats: {
				included: plan.seats.included,
				// Negotiated caps win; every plan has a concrete ceiling.
				max: quota?.limits.members ?? plan.seats.max,
				monthlyUsd: plan.seats.monthlyUsd,
			},
			features: plan.features,
		},
		proPlan: {
			monthlyUsd: PLANS.pro.monthlyUsd,
			annualMonthlyUsd: PLANS.pro.annualMonthlyUsd,
			seatsIncluded: PLANS.pro.seats.included,
			extraSeatUsd: PLANS.pro.seats.monthlyUsd,
			features: PLANS.pro.features,
		},
		subscription: subscription ?? null,
		seats,
		currentMonthlyUsd: priceForSeats(plan.id, seats),
		delivery: quota?.delivery ?? null,
		deliveryOverageUsdPerGb: DELIVERY_OVERAGE_USD_PER_GB,
		// Only the owner may attach billing to a workspace.
		isOwner: activeOrganization.role === "owner",
		billingConfigured: Boolean(polarProductIdFor("pro")),
		workspace: { id: activeOrganization.id, name: activeOrganization.name },
		billingContactIsMe: subscription ? subscription.userId === user.id : true,
	};
};
