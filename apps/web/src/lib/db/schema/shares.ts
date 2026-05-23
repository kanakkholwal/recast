import {
	boolean,
	index,
	integer,
	pgEnum,
	pgTable,
	text,
	timestamp,
} from "drizzle-orm/pg-core";
import { user } from "./auth";
import { organization } from "./organization";
import { recast } from "./recasts";

/**
 * Who can open a share link:
 *   - `public`  → anyone with the URL (default; matches v1 behavior)
 *   - `team`    → signed-in members of `organizationId` only
 *   - `private` → owner + global admins only
 *
 * Visibility is changeable from the share page itself by the owner or a
 * platform admin. Team-scoped shares require an `organizationId`; the
 * loader treats a team share with no org id as effectively private.
 */
export const shareVisibilityEnum = pgEnum("share_visibility", [
	"public",
	"team",
	"private",
]);

export const share = pgTable(
	"share",
	{
		slug: text("slug").primaryKey(),
		recastId: text("recast_id")
			.notNull()
			.references(() => recast.id, { onDelete: "cascade" }),
		ownerId: text("owner_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		/** Owning team — required for `team` visibility, optional otherwise. */
		organizationId: text("organization_id").references(() => organization.id, {
			onDelete: "set null",
		}),
		visibility: shareVisibilityEnum("visibility").notNull().default("public"),
		passwordHash: text("password_hash"),
		expiresAt: timestamp("expires_at"),
		// On free plan this is always true and shown on the player; Pro removes.
		watermark: boolean("watermark").notNull().default(true),
		// Cached counter — incremented from share_view writes for cheap reads.
		viewsCount: integer("views_count").notNull().default(0),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_owner_idx").on(t.ownerId),
		index("share_recast_idx").on(t.recastId),
		index("share_org_idx").on(t.organizationId),
	],
);

/**
 * Append-only event log. Hot for writes, dense over time. Kept in the same
 * Postgres for v1; partition by created_at month or move to ClickHouse if
 * it grows past tens of millions of rows.
 */
export const shareView = pgTable(
	"share_view",
	{
		id: text("id").primaryKey(),
		shareId: text("share_id")
			.notNull()
			.references(() => share.slug, { onDelete: "cascade" }),
		// Anonymous fingerprint — not tied to user.id (viewers don't need accounts).
		sessionId: text("session_id").notNull(),
		country: text("country"),
		userAgent: text("user_agent"),
		watchPct: integer("watch_pct").notNull().default(0),
		completed: boolean("completed").notNull().default(false),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_view_share_idx").on(t.shareId),
		index("share_view_created_idx").on(t.createdAt),
	],
);

export type Share = typeof share.$inferSelect;
export type ShareView = typeof shareView.$inferSelect;
