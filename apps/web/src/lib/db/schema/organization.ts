import { index, pgTable, text, timestamp, unique } from "drizzle-orm/pg-core";
import { user } from "./auth";

/**
 * Tables managed by Better Auth's `organization` plugin. Column names follow
 * the plugin's defaults so the adapter resolves them without remapping.
 *
 * Application-owned columns:
 *   - `organization.plan` — "free" | "pro" | "enterprise". Drives member
 *     caps (3 / 50 / unlimited) via `membershipLimit` in auth/server.ts.
 *     **Only changeable from /admin/teams/[id]** — no self-serve checkout.
 *   - `organization.memberCap` — derived snapshot, kept on the row so caps
 *     can be enforced without re-reading the plan→cap map in hot paths.
 */

export const organization = pgTable("organization", {
	id: text("id").primaryKey(),
	name: text("name").notNull(),
	slug: text("slug").notNull().unique(),
	logo: text("logo"),
	/** JSON-encoded blob the plugin stores as text. */
	metadata: text("metadata"),
	/** Plan — "free" (default), "pro", "enterprise". Admin-managed. */
	plan: text("plan").notNull().default("free"),
	createdAt: timestamp("created_at").notNull().defaultNow(),
});

export const member = pgTable(
	"member",
	{
		id: text("id").primaryKey(),
		organizationId: text("organization_id")
			.notNull()
			.references(() => organization.id, { onDelete: "cascade" }),
		userId: text("user_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		/** Plugin defaults: "owner" | "admin" | "member". */
		role: text("role").notNull().default("member"),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("member_org_idx").on(t.organizationId),
		index("member_user_idx").on(t.userId),
		// One row per (org, user) pair. Prevents duplicate memberships from
		// botched invite-accept retries or concurrent addMember races — both
		// the seat-count cap and role updates require this to be unambiguous.
		unique("member_organization_user_key").on(t.organizationId, t.userId),
	],
);

export const invitation = pgTable(
	"invitation",
	{
		id: text("id").primaryKey(),
		organizationId: text("organization_id")
			.notNull()
			.references(() => organization.id, { onDelete: "cascade" }),
		email: text("email").notNull(),
		role: text("role").notNull().default("member"),
		/** Plugin lifecycle: pending → accepted | rejected | canceled. */
		status: text("status").notNull().default("pending"),
		expiresAt: timestamp("expires_at"),
		inviterId: text("inviter_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("invitation_org_idx").on(t.organizationId),
		index("invitation_email_idx").on(t.email),
	],
);

export type Organization = typeof organization.$inferSelect;
export type Member = typeof member.$inferSelect;
export type Invitation = typeof invitation.$inferSelect;

/** Plan tier → member cap. Single source of truth read everywhere. */
export const TEAM_PLAN_MEMBER_CAPS: Record<string, number> = {
	free: 3,
	pro: 50,
	enterprise: Number.POSITIVE_INFINITY,
};

/** Team-count cap per user, by their highest team plan owned. */
export const USER_TEAM_OWNERSHIP_CAPS = {
	/** Free user (owns only free teams): can own this many total. */
	free: 3,
	/** User who owns at least one Pro/Enterprise team. */
	paid: 10,
} as const;
