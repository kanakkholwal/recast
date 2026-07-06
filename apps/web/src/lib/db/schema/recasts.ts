import {
	bigint,
	index,
	integer,
	pgEnum,
	pgTable,
	text,
	timestamp,
} from "drizzle-orm/pg-core";
import { user } from "./auth";
import { folder } from "./folders";
import { organization } from "./organization";

export const recastSourceEnum = pgEnum("recast_source", ["cloud", "local"]);

/**
 * Lifecycle status. Drives the expiry job and dashboard filters.
 *   - `draft`     → uploaded, owner hasn't published a share yet
 *   - `published` → at least one share link exists, file is hot in R2
 *   - `archived`  → blob deleted by expiry job (0 views ≥14d); row + poster kept
 *                   so the owner can re-upload from desktop. Hard-deleted at
 *                   day 30 via `deletedAt`.
 */
export const recastStatusEnum = pgEnum("recast_status", [
	"draft",
	"published",
	"archived",
]);

export const recast = pgTable(
	"recast",
	{
		id: text("id").primaryKey(),
		/**
		 * Owning workspace. Recasts are workspace-first — there is no
		 * "personal" surface; a solo user owns a workspace of 1, auto-created
		 * on signup. Cascade so deleting an org wipes its content.
		 */
		workspaceId: text("workspace_id")
			.notNull()
			.references(() => organization.id, { onDelete: "cascade" }),
		ownerId: text("owner_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		/** Nullable = root of the workspace. */
		folderId: text("folder_id").references(() => folder.id, {
			onDelete: "set null",
		}),
		title: text("title").notNull(),
		/** Optional viewer-facing blurb — shown under the video on the share
		 *  page and used as the OG/social-card description. Null until the owner
		 *  writes one. */
		description: text("description"),
		durationSec: integer("duration_sec").notNull().default(0),
		sizeBytes: bigint("size_bytes", { mode: "number" }).notNull().default(0),
		/** Resolution stored in R2 (post-downscale on Free). */
		width: integer("width"),
		height: integer("height"),
		fps: integer("fps"),
		videoUrl: text("video_url").notNull(),
		posterUrl: text("poster_url"),
		/** WebVTT captions track object key (bare key), or null. Loaded by the
		 *  player as a `<track>`. Written on upload/complete when the desktop
		 *  app sent a caption track. */
		captionsUrl: text("captions_url"),
		source: recastSourceEnum("source").notNull().default("cloud"),
		status: recastStatusEnum("status").notNull().default("draft"),
		/** "cloudinary" | "r2" | "s3" | null when local. */
		provider: text("provider"),
		/**
		 * Denormalized last-view timestamp. Updated when a `share_view` row is
		 * inserted for any share of this recast. Indexed for the expiry sweep
		 * (find recasts with no views in N days). NULL = never viewed.
		 */
		lastViewedAt: timestamp("last_viewed_at"),
		/** Set when the expiry job moves the blob to archive (file gone, row kept). */
		archivedAt: timestamp("archived_at"),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
		deletedAt: timestamp("deleted_at"),
	},
	(t) => [
		index("recast_workspace_idx").on(t.workspaceId),
		index("recast_workspace_folder_idx").on(t.workspaceId, t.folderId),
		index("recast_workspace_status_idx").on(t.workspaceId, t.status),
		index("recast_owner_idx").on(t.ownerId),
		index("recast_owner_created_idx").on(t.ownerId, t.createdAt),
		// Expiry sweep: WHERE status='published' AND last_viewed_at < now - 14d.
		index("recast_status_last_viewed_idx").on(t.status, t.lastViewedAt),
	],
);

export type Recast = typeof recast.$inferSelect;
