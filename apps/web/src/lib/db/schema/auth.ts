import { boolean, pgTable, text, timestamp } from "drizzle-orm/pg-core";

/**
 * Better Auth core tables. Names and shapes follow the documented schema —
 * don't rename columns without also re-running `betterAuth.cli` migrations,
 * or the adapter won't find them.
 *
 * `role`, `banned`, `banReason`, `banExpires` are owned by Better Auth's
 * `admin` plugin; the plugin manages them through the admin API endpoints.
 * `status` is application-owned (waitlist gating, kept separate from `role`).
 */

export const user = pgTable("user", {
	id: text("id").primaryKey(),
	name: text("name").notNull(),
	email: text("email").notNull().unique(),
	emailVerified: boolean("email_verified").notNull().default(false),
	image: text("image"),
	/** Managed by `better-auth/plugins/admin`. Defaults: "user" | "admin". */
	role: text("role").notNull().default("user"),
	/**
	 * Application status, separate from `role`. `pending` = on the waitlist
	 * and not yet activated; magic link + password reset short-circuit when
	 * this is `pending`. Flip to `active` from the admin dashboard
	 * (or `/admin/waitlist` for bulk approve).
	 */
	status: text("status").notNull().default("active"),
	/** Admin plugin ban fields — managed via /admin/ban-user. */
	banned: boolean("banned"),
	banReason: text("ban_reason"),
	banExpires: timestamp("ban_expires"),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const session = pgTable("session", {
	id: text("id").primaryKey(),
	expiresAt: timestamp("expires_at").notNull(),
	token: text("token").notNull().unique(),
	ipAddress: text("ip_address"),
	userAgent: text("user_agent"),
	userId: text("user_id")
		.notNull()
		.references(() => user.id, { onDelete: "cascade" }),
	/**
	 * Admin plugin impersonation marker. When set, this session was created
	 * by an admin calling /admin/impersonate-user; `stopImpersonating()`
	 * restores the original admin session.
	 */
	impersonatedBy: text("impersonated_by"),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const account = pgTable("account", {
	id: text("id").primaryKey(),
	accountId: text("account_id").notNull(),
	providerId: text("provider_id").notNull(),
	userId: text("user_id")
		.notNull()
		.references(() => user.id, { onDelete: "cascade" }),
	accessToken: text("access_token"),
	refreshToken: text("refresh_token"),
	idToken: text("id_token"),
	accessTokenExpiresAt: timestamp("access_token_expires_at"),
	refreshTokenExpiresAt: timestamp("refresh_token_expires_at"),
	scope: text("scope"),
	password: text("password"),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const verification = pgTable("verification", {
	id: text("id").primaryKey(),
	identifier: text("identifier").notNull(),
	value: text("value").notNull(),
	expiresAt: timestamp("expires_at").notNull(),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export type User = typeof user.$inferSelect;
export type Session = typeof session.$inferSelect;
