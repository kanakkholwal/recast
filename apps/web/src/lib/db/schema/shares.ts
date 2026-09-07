import {
	boolean,
	index,
	integer,
	pgEnum,
	pgTable,
	text,
	timestamp,
	unique,
} from "drizzle-orm/pg-core";
import { user } from "./auth";
import { organization } from "./organization";
import { recast } from "./recasts";

/**
 * Who can open a share link (Google-Docs-style scope):
 *   - `private`   → owner + workspace admins only
 *   - `workspace` → any signed-in member of `organizationId`
 *   - `selected`  → owner + addresses listed in `share_member`
 *   - `public`    → anyone with the URL
 *
 * `team` is kept as an alias for `workspace` to not break older rows /
 * loaders — new code should write `workspace`.
 *
 * Password is orthogonal — applies on top of any visibility above `private`.
 */
export const shareVisibilityEnum = pgEnum("share_visibility", [
	"private",
	"workspace",
	"team",
	"selected",
	"public",
]);

export const shareMemberRoleEnum = pgEnum("share_member_role", ["viewer", "commenter"]);

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
		/** Required for `workspace`/`team` visibility, optional otherwise. */
		organizationId: text("organization_id").references(() => organization.id, {
			onDelete: "set null",
		}),
		visibility: shareVisibilityEnum("visibility").notNull().default("public"),
		passwordHash: text("password_hash"),
		expiresAt: timestamp("expires_at"),
		/** Free plan: always true and shown on player. Pro removes. */
		/**
		 * Owner's call-to-action — the "next step" a founder wants the viewer
		 * to take (book a call, try it, reply). Rendered as a persistent
		 * button below the video AND an end-card overlay when playback ends.
		 * Both null = no CTA. `ctaUrl` is an absolute URL; `ctaLabel` is the
		 * button text.
		 */
		ctaLabel: text("cta_label"),
		ctaUrl: text("cta_url"),
		/**
		 * Whether viewers can post comments on this share. Reactions are
		 * always allowed (lighter, less abuse surface). The create flow sets
		 * this `false` for Pro workspaces per the Cloud-plan default; column
		 * default is `true` for Free.
		 */
		commentsEnabled: boolean("comments_enabled").notNull().default(true),
		/** Cached counter — incremented from share_view writes for cheap reads. */
		viewsCount: integer("views_count").notNull().default(0),
		/**
		 * Owner opt-in to let search engines index this share. Only meaningful for
		 * `public` visibility. Default false: a public link stays shareable but not
		 * crawlable unless the owner explicitly lists it, so ad-hoc recordings never
		 * surface in search by surprise.
		 */
		searchable: boolean("searchable").notNull().default(false),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_owner_idx").on(t.ownerId),
		index("share_recast_idx").on(t.recastId),
		index("share_org_idx").on(t.organizationId),
	],
);

/**
 * Per-share allowlist for `visibility = 'selected'`. Supports inviting either
 * an existing user (resolved `userId`) or any email (signs in via magic link
 * on first view). Email is always present so removal-by-email works even
 * after the user signs up.
 */
export const shareMember = pgTable(
	"share_member",
	{
		id: text("id").primaryKey(),
		shareSlug: text("share_slug")
			.notNull()
			.references(() => share.slug, { onDelete: "cascade" }),
		email: text("email").notNull(),
		/** Resolved on invite if the email already maps to a user. */
		userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
		role: shareMemberRoleEnum("role").notNull().default("viewer"),
		invitedBy: text("invited_by")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_member_share_idx").on(t.shareSlug),
		index("share_member_user_idx").on(t.userId),
		unique("share_member_share_email_key").on(t.shareSlug, t.email),
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
		/** Anonymous fingerprint — not tied to user.id (viewers don't need accounts). */
		sessionId: text("session_id").notNull(),
		country: text("country"),
		userAgent: text("user_agent"),
		/**
		 * Normalized device class derived from the user-agent at ingest:
		 * `"mobile" | "tablet" | "desktop"`. Stored (rather than parsed on every
		 * read) so audience breakdowns are a cheap GROUP BY; historical rows with
		 * a null `device` fall back to a UA re-parse at read time.
		 */
		device: text("device"),
		/**
		 * Where the viewer arrived from — the hostname of `document.referrer`,
		 * sent by the player on the "start" beacon (the request `Referer` header
		 * is always the share page itself, so it can't tell us this). Null for
		 * direct opens / privacy-stripped referrers.
		 */
		referrer: text("referrer"),
		watchPct: integer("watch_pct").notNull().default(0),
		completed: boolean("completed").notNull().default(false),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_view_share_idx").on(t.shareId),
		index("share_view_created_idx").on(t.createdAt),
	],
);

/**
 * Viewer comments on a share. Flat (no threading in v1) and anchored to a
 * point in the video via `atSeconds`. Comments are name-only: a viewer
 * doesn't need a Recast account, so identity is a self-supplied
 * `authorName` plus the anonymous `sessionId` fingerprint (same one
 * `share_view` uses). Soft-deleted via `deletedAt` so the owner can
 * moderate without breaking reply context or counts.
 */
export const shareComment = pgTable(
	"share_comment",
	{
		id: text("id").primaryKey(),
		shareSlug: text("share_slug")
			.notNull()
			.references(() => share.slug, { onDelete: "cascade" }),
		/** Anonymous fingerprint — lets the author edit/remove their own posts. */
		sessionId: text("session_id").notNull(),
		authorName: text("author_name").notNull(),
		/** Set when a signed-in account posted this — server-stamped from the
		 *  session (never client input), so it can't be spoofed. Drives the
		 *  "verified" badge; null for guest (name-only) comments. Set-null on
		 *  account delete so the comment survives, just unbadged. */
		authorUserId: text("author_user_id").references(() => user.id, {
			onDelete: "set null",
		}),
		/** Point in the video the comment is anchored to. */
		atSeconds: integer("at_seconds").notNull().default(0),
		body: text("body").notNull(),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		/** Owner moderation — soft delete keeps counts stable. */
		deletedAt: timestamp("deleted_at"),
	},
	(t) => [
		index("share_comment_share_idx").on(t.shareSlug),
		index("share_comment_share_created_idx").on(t.shareSlug, t.createdAt),
	],
);

/**
 * Lightweight sentiment reactions (Cap-style). Always allowed regardless of
 * the comments toggle. A viewer gets ONE reaction per share — picking a second
 * emoji switches it in place — and the reactor is keyed by IP (`ipHash`), so
 * one IP maps to one reaction. `atSeconds` records WHERE in the video the
 * viewer was when they reacted — surfaced to the owner later ("most viewers
 * loved 0:52"), not used by the viewer-facing toggle.
 */
export const shareReaction = pgTable(
	"share_reaction",
	{
		id: text("id").primaryKey(),
		shareSlug: text("share_slug")
			.notNull()
			.references(() => share.slug, { onDelete: "cascade" }),
		sessionId: text("session_id").notNull(),
		/**
		 * Salted hash of the reactor's IP (or their session when the IP can't be
		 * resolved) — the dedup identity. Nullable only for legacy rows written
		 * before single-reaction/IP mapping existed.
		 */
		ipHash: text("ip_hash"),
		emoji: text("emoji").notNull(),
		/** Playhead position when the reaction was made (owner-insight metadata). */
		atSeconds: integer("at_seconds").notNull().default(0),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("share_reaction_share_idx").on(t.shareSlug),
		// One reaction per share and reactor, replacing the key that let a viewer stack multiple emojis.
		unique("share_reaction_reactor_key").on(t.shareSlug, t.ipHash),
	],
);

export type Share = typeof share.$inferSelect;
export type ShareMember = typeof shareMember.$inferSelect;
export type ShareView = typeof shareView.$inferSelect;
export type ShareComment = typeof shareComment.$inferSelect;
export type ShareReaction = typeof shareReaction.$inferSelect;
