import {
	boolean,
	pgEnum,
	pgTable,
	text,
	timestamp,
} from "drizzle-orm/pg-core";
import { user } from "./auth";

export const planEnum = pgEnum("plan", ["free", "pro"]);
export const subscriptionStatusEnum = pgEnum("subscription_status", [
	"active",
	"canceled",
	"past_due",
	"incomplete",
	"trialing",
	"unpaid",
]);

/**
 * One row per user. Webhooks from Polar are the source of truth — this table
 * mirrors the subscription state for fast lookups (e.g. "is this user on
 * Pro?" without hitting the Polar API on every request).
 */
export const subscription = pgTable("subscription", {
	id: text("id").primaryKey(),
	userId: text("user_id")
		.notNull()
		.unique()
		.references(() => user.id, { onDelete: "cascade" }),
	polarCustomerId: text("polar_customer_id"),
	polarSubscriptionId: text("polar_subscription_id"),
	plan: planEnum("plan").notNull().default("free"),
	status: subscriptionStatusEnum("status").notNull().default("active"),
	currentPeriodEnd: timestamp("current_period_end"),
	cancelAtPeriodEnd: boolean("cancel_at_period_end").notNull().default(false),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export type Subscription = typeof subscription.$inferSelect;
