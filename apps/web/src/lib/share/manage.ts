import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { member, recast, share, user } from "$lib/db/schema";

/**
 * Resolve whether `userId` may MANAGE a share — change its visibility/settings,
 * moderate comments, etc. A manager is any of:
 *   1. the share's creator (`share.ownerId`),
 *   2. an owner/admin MEMBER of the recast's workspace (`member.role`), or
 *   3. a global admin (`user.role === "admin"`).
 *
 * Clause 2 is the team rule: a workspace's owners/admins govern shares created
 * by their teammates, not just the original author. The workspace is the
 * recast's `workspaceId` (the owning org), NOT `share.organizationId` — the
 * latter is only set for workspace-visibility shares, whereas a private share
 * of a workspace recast should still be manageable by that workspace's admins.
 *
 * Returns `null` when the share doesn't exist. Otherwise the share's `ownerId`
 * and the recast's `workspaceId` (handy for callers) plus the `canManage`
 * verdict. Roles are read from the DB (not the session) so a role change takes
 * effect on the next request.
 */
export async function resolveShareManage(
	slug: string,
	userId: string | null | undefined,
): Promise<{ ownerId: string; workspaceId: string; recastId: string; canManage: boolean } | null> {
	const db = getDb();
	const [row] = await db
		.select({ ownerId: share.ownerId, workspaceId: recast.workspaceId, recastId: recast.id })
		.from(share)
		.innerJoin(recast, eq(share.recastId, recast.id))
		.where(eq(share.slug, slug))
		.limit(1);
	if (!row) return null;
	if (!userId) return { ...row, canManage: false };

	// 1. Share creator.
	if (row.ownerId === userId) return { ...row, canManage: true };

	// 2. Owner/admin of the recast's workspace.
	const [m] = await db
		.select({ role: member.role })
		.from(member)
		.where(and(eq(member.userId, userId), eq(member.organizationId, row.workspaceId)))
		.limit(1);
	if (m && (m.role === "owner" || m.role === "admin")) {
		return { ...row, canManage: true };
	}

	// 3. Global admin.
	const [u] = await db.select({ role: user.role }).from(user).where(eq(user.id, userId)).limit(1);
	return { ...row, canManage: u?.role === "admin" };
}
