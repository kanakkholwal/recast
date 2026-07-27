import { integer, pgTable, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { organization } from "./organization";

/**
 * Which workspace a user is buying for. Polar webhooks only carry the customer
 * (a user), so without this a payment can't be attributed to a workspace when
 * the buyer owns more than one. Written on checkout start, consumed on webhook.
 */
export const billingCheckoutIntent = pgTable("billing_checkout_intent", {
	userId: text("user_id")
		.primaryKey()
		.references(() => user.id, { onDelete: "cascade" }),
	organizationId: text("organization_id")
		.notNull()
		.references(() => organization.id, { onDelete: "cascade" }),
	seats: integer("seats").notNull().default(3),
	createdAt: timestamp("created_at").notNull().defaultNow(),
});

export type BillingCheckoutIntent = typeof billingCheckoutIntent.$inferSelect;
