/**
 * Who may see and act on a recast. One predicate for the loaders and the API,
 * so the library list, the detail page and the mutation endpoints can't drift
 * into showing something you aren't allowed to open.
 */

/** Workspace roles that act on any recast in the workspace. */
export function isWorkspaceManager(role: string | null | undefined): boolean {
	return role === "owner" || role === "admin";
}

/** The recast's creator, a workspace owner/admin, or a platform admin. */
export function canAccessRecast(opts: {
	recastOwnerId: string;
	userId: string;
	workspaceRole?: string | null;
	platformRole?: string | null;
}): boolean {
	return (
		opts.recastOwnerId === opts.userId ||
		isWorkspaceManager(opts.workspaceRole) ||
		opts.platformRole === "admin"
	);
}
