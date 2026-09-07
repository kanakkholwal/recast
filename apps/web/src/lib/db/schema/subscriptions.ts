import { boolean, index, integer, pgEnum, pgTable, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { organization } from "./organization";

export const planEnum = pgEnum("plan", ["free", "pro", "enterprise"]);
export const subscriptionStatusEnum = pgEnum("subscription_status", [
	"active",
	"canceled",
	"past_due",
	"incomplete",
	"trialing",
	"unpaid",
]);

/**
 * One row per **workspace** — workspaces are the billing unit, since seats and
 * quota are workspace-scoped. `userId` is the billing contact, not the subject.
 */
export const subscription = pgTable(
	"subscription",
	{
		id: text("id").primaryKey(),
		organizationId: text("organization_id")
			.notNull()
			.unique()
			.references(() => organization.id, { onDelete: "cascade" }),
		/** Who pays — owns the Polar customer record and the portal session. */
		userId: text("user_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		polarCustomerId: text("polar_customer_id"),
		polarSubscriptionId: text("polar_subscription_id"),
		plan: planEnum("plan").notNull().default("free"),
		status: subscriptionStatusEnum("status").notNull().default("active"),
		/** Seats billed this period; base price covers `plan.seats.included`. */
		seats: integer("seats").notNull().default(3),
		currentPeriodEnd: timestamp("current_period_end"),
		cancelAtPeriodEnd: boolean("cancel_at_period_end").notNull().default(false),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [index("subscription_user_idx").on(t.userId)],
);

export type Subscription = typeof subscription.$inferSelect;
