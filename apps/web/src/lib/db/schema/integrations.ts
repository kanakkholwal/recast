import {
	index,
	jsonb,
	pgEnum,
	pgTable,
	text,
	timestamp,
	uniqueIndex,
} from "drizzle-orm/pg-core";
import { user } from "./auth";

export const integrationProviderEnum = pgEnum("integration_provider", [
	"cloudinary",
	"s3",
]);

/**
 * BYO storage credentials (Cloudinary, S3). The `config` payload should be
 * encrypted at the app layer before being persisted — see
 * `src/lib/billing/sync.ts`-adjacent helpers when we add the crypto utility.
 */
export const integration = pgTable(
	"integration",
	{
		id: text("id").primaryKey(),
		userId: text("user_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		provider: integrationProviderEnum("provider").notNull(),
		config: jsonb("config").$type<Record<string, string>>().notNull(),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [
		index("integration_user_idx").on(t.userId),
		uniqueIndex("integration_user_provider_idx").on(t.userId, t.provider),
	],
);

export type Integration = typeof integration.$inferSelect;
