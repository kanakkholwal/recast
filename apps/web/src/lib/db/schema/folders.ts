import { index, pgEnum, pgTable, text, timestamp, unique } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { member, organization } from "./organization";

/**
 * Per-folder permission override role. Absence of a `folder_permission` row
 * for a given (folder, member) pair means **inherit from parent** (which
 * may itself inherit, terminating at the workspace's default access for
 * the member's workspace role).
 */
export const folderPermissionRoleEnum = pgEnum("folder_permission_role", [
	"viewer",
	"editor",
	"admin",
	"no_access",
]);

/**
 * Workspace-scoped folder for organizing recasts. Nested via `parentId`;
 * deleting a parent cascades to children. Recasts move to root
 * (`recast.folder_id = NULL`) when their folder is deleted — see
 * `recast.folderId` (SET NULL).
 *
 * `path` is a denormalized materialized path ("/", "/marketing/",
 * "/marketing/onboarding/") used for breadcrumb queries without recursive
 * CTEs. Kept in app code on rename/move.
 */
export const folder = pgTable(
	"folder",
	{
		id: text("id").primaryKey(),
		workspaceId: text("workspace_id")
			.notNull()
			.references(() => organization.id, { onDelete: "cascade" }),
		parentId: text("parent_id"),
		name: text("name").notNull(),
		/** Hex color (e.g. "#7c3aed") or null for default. */
		color: text("color"),
		/** Materialized path including this folder. Updated on rename/move. */
		path: text("path").notNull().default("/"),
		createdBy: text("created_by")
			.notNull()
			.references(() => user.id, { onDelete: "cascade" }),
		createdAt: timestamp("created_at").notNull().defaultNow(),
		updatedAt: timestamp("updated_at").notNull().defaultNow(),
	},
	(t) => [
		index("folder_workspace_idx").on(t.workspaceId),
		index("folder_workspace_parent_idx").on(t.workspaceId, t.parentId),
		index("folder_workspace_path_idx").on(t.workspaceId, t.path),
		// Siblings need distinct names, but Postgres treats NULLs as distinct, so the app must collapse a null parent to a sentinel first.
		unique("folder_parent_name_key").on(t.workspaceId, t.parentId, t.name),
	],
);

/**
 * Permission override. Only insert a row when the member's access on this
 * folder differs from what they'd inherit. Empty = inherit. Use
 * `role = 'no_access'` to explicitly revoke access that would otherwise
 * be inherited.
 */
export const folderPermission = pgTable(
	"folder_permission",
	{
		id: text("id").primaryKey(),
		folderId: text("folder_id")
			.notNull()
			.references(() => folder.id, { onDelete: "cascade" }),
		memberId: text("member_id")
			.notNull()
			.references(() => member.id, { onDelete: "cascade" }),
		role: folderPermissionRoleEnum("role").notNull(),
		createdAt: timestamp("created_at").notNull().defaultNow(),
	},
	(t) => [
		index("folder_permission_folder_idx").on(t.folderId),
		index("folder_permission_member_idx").on(t.memberId),
		unique("folder_permission_folder_member_key").on(t.folderId, t.memberId),
	],
);

export type Folder = typeof folder.$inferSelect;
export type FolderPermission = typeof folderPermission.$inferSelect;
