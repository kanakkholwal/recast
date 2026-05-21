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

export const recastSourceEnum = pgEnum("recast_source", ["cloud", "local"]);

export const recast = pgTable(
	"recast",
	{
		id: text("id").primaryKey(),
		ownerId: text("owner_id")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		title: text("title").notNull(),
		durationSec: integer("duration_sec").notNull().default(0),
		sizeBytes: bigint("size_bytes", { mode: "number" }).notNull().default(0),
		videoUrl: text("video_url").notNull(),
		posterUrl: text("poster_url"),
		source: recastSourceEnum("source").notNull().default("cloud"),
		// e.g. "cloudinary" | "r2" | "s3" | null when local
		provider: text("provider"),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
		deletedAt: timestamp("deleted_at"),
	},
	(t) => [
		index("recast_owner_idx").on(t.ownerId),
		index("recast_owner_created_idx").on(t.ownerId, t.createdAt),
	],
);

export type Recast = typeof recast.$inferSelect;
