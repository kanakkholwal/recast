import {
	bigint,
	index,
	integer,
	pgTable,
	text,
	timestamp,
} from "drizzle-orm/pg-core";
import { limitsFor } from "$lib/billing/catalog";
import { organization } from "./organization";

/**
 * Per-workspace cached usage counters for the billing surface. One row per
 * workspace, upserted in the same transaction as every recast/view mutation.
 */
export const workspaceUsage = pgTable(
	"workspace_usage",
	{
		workspaceId: text("workspace_id")
			.primaryKey()
			.references(() => organization.id, { onDelete: "cascade" }),
		/** Sum of `recast.size_bytes` for non-archived, non-deleted recasts. */
		storageBytes: bigint("storage_bytes", { mode: "number" })
			.notNull()
			.default(0),
		/** Count of recasts with status='published' (counts toward link cap). */
		activeRecastsCount: integer("active_recasts_count").notNull().default(0),
		/** Count of recasts in `archived` state — billed at $0 but visible to owner. */
		archivedRecastsCount: integer("archived_recasts_count").notNull().default(0),
		/** Workspace member count, kept in sync from `member` inserts/deletes. */
		membersCount: integer("members_count").notNull().default(1),
		/**
		 * Bytes served to viewers in the current billing month. Bumped per view
		 * start by the recast's size — an upper bound, since partial watches
		 * transfer less. Conservative on purpose: it protects the egress bill.
		 */
		deliveryBytesThisMonth: bigint("delivery_bytes_this_month", {
			mode: "number",
		})
			.notNull()
			.default(0),
		/** Start of the window `deliveryBytesThisMonth` covers; reset rolls it forward. */
		deliveryPeriodStart: timestamp("delivery_period_start")
			.notNull()
			.defaultNow(),
		/**
		 * Rolling 30d view count across all shares in the workspace. Used for
		 * the analytics overview card and abuse detection (sudden spike).
		 */
		viewsLast30d: integer("views_last_30d").notNull().default(0),
		lastRecalculatedAt: timestamp("last_recalculated_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [
		// Quota-warning sweep: workspaces past 80% / 100% of their plan cap.
		index("workspace_usage_storage_idx").on(t.storageBytes),
		// Delivery-cap sweep runs on the same cadence, filtered by period start.
		index("workspace_usage_delivery_idx").on(t.deliveryPeriodStart),
	],
);

/** Enforcement view of the plan catalog. Edit `$lib/billing/catalog.ts`, not this. */
export const QUOTA = {
	free: limitsFor("free"),
	pro: limitsFor("pro"),
	enterprise: limitsFor("enterprise"),
} as const;

export type WorkspaceUsage = typeof workspaceUsage.$inferSelect;
