import { index, pgTable, text, timestamp } from "drizzle-orm/pg-core";

/**
 * Waitlist captures. In production /signup redirects here; in dev it stays
 * accessible for testing the funnel. `source` lets us track where the email
 * came from (e.g. "signup-redirect", "homepage").
 */
export const waitlist = pgTable(
	"waitlist",
	{
		id: text("id").primaryKey(),
		email: text("email").notNull().unique(),
		source: text("source"),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [index("waitlist_created_idx").on(t.createdAt)],
);

export type Waitlist = typeof waitlist.$inferSelect;
