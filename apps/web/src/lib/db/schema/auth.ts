import {
	boolean,
	index,
	integer,
	pgTable,
	text,
	timestamp,
} from "drizzle-orm/pg-core";
import { organization } from "./organization";

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
	defaultWorkspaceId: text("default_workspace_id").references(
		() => organization.id,
		{ onDelete: "set null" },
	),
	createdAt: timestamp("created_at").notNull().defaultNow(),
	updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const session = pgTable(
	"session",
	{
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
		/**
		 * Active organization for this session — managed by Better Auth's
		 * `organization` plugin via `setActiveOrganization`. Dashboard data
		 * scopes to this id.
		 *
		 * FK with `ON DELETE SET NULL` so deleting an org clears the pointer
		 * rather than orphaning the session row or cascading the session away.
		 * Lazy `() => organization.id` keeps the auth/organization import
		 * cycle resolvable (Drizzle's table-definition phase reads the column
		 * via the callback, not eagerly at module load).
		 */
		activeOrganizationId: text("active_organization_id").references(
			() => organization.id,
			{ onDelete: "set null" },
		),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [
		index("session_active_org_idx").on(t.activeOrganizationId),
	],
);

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

/**
 * Owned by Better Auth's `device-authorization` plugin. Holds the in-flight
 * OAuth 2.0 Device Authorization Grant (RFC 8628) requests issued from the
 * desktop app:
 *
 *   - desktop POSTs /device/code → row inserted (status: "pending", no userId)
 *   - browser GETs /device?user_code=... → userId populated from session
 *   - browser POSTs /device/approve → status flips to "approved"
 *   - desktop POSTs /device/token → row deleted, real `session` row issued
 *
 * Field names match the plugin's expected schema exactly — renaming will
 * break the adapter's column lookups. `userId` is nullable because the row
 * exists before the user has claimed it.
 */
export const deviceCode = pgTable(
	"deviceCode",
	{
		id: text("id").primaryKey(),
		deviceCode: text("device_code").notNull(),
		userCode: text("user_code").notNull(),
		userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
		expiresAt: timestamp("expires_at").notNull(),
		status: text("status").notNull(),
		lastPolledAt: timestamp("last_polled_at"),
		pollingInterval: integer("polling_interval"),
		clientId: text("client_id"),
		scope: text("scope"),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [
		// Plugin looks rows up by deviceCode (on /device/token poll) and by
		// userCode (on /device GET + approve/deny). Both are hot paths.
		index("device_code_device_code_idx").on(t.deviceCode),
		index("device_code_user_code_idx").on(t.userCode),
	],
);

export type User = typeof user.$inferSelect;
export type Session = typeof session.$inferSelect;
export type DeviceCode = typeof deviceCode.$inferSelect;
