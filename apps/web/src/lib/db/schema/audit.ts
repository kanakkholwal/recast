import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

/**
 * Admin audit log. Written explicitly from each `/admin/*` action wrapper
 * (see `$lib/admin/audit.ts`) so we can capture the *actor* (the admin who
 * performed the action) and not just the target row change.
 *
 * Append-only — never UPDATE or DELETE rows here. If you need to remove an
 * entry for compliance, write a counter-entry instead.
 */

export const auditLog = pgTable(
	"audit_log",
	{
		id: text("id").primaryKey(),
		actorId: text("actor_id").notNull(),
		action: text("action").notNull(),
		targetUserId: text("target_user_id"),
		metadata: jsonb("metadata").$type<Record<string, unknown>>(),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("audit_actor_idx").on(t.actorId),
		index("audit_target_idx").on(t.targetUserId),
		index("audit_created_idx").on(t.createdAt),
	],
);

export type AuditLog = typeof auditLog.$inferSelect;

/**
 * Discriminated set of audit actions we record. Add new ones at the bottom —
 * never repurpose an existing string, the column is the canonical key.
 */
export const AUDIT_ACTIONS = [
	"user.create",
	"user.update",
	"user.delete",
	"user.set_role",
	"user.set_password",
	"user.ban",
	"user.unban",
	"user.impersonate",
	"user.stop_impersonate",
	"session.revoke",
	"waitlist.approve",
	"team.update_plan",
	"team.rename",
] as const;

export type AuditAction = (typeof AUDIT_ACTIONS)[number];
