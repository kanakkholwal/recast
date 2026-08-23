import { error } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { canAccessRecast } from "$lib/dashboard/access";
import { getDb } from "$lib/db";
import { member, recast, user } from "$lib/db/schema";

/**
 * One authorization gate for every `/api/recasts/[id]/*` handler. Five routes
 * had five copies of it that had already drifted apart (share was owner-only,
 * the rest owner-or-platform-admin, none of them let a workspace owner manage
 * their own workspace's content).
 */

export type AuthorizedRecast = {
	id: string;
	ownerId: string;
	workspaceId: string;
	videoUrl: string;
	posterUrl: string | null;
	sizeBytes: number;
	status: string;
	/** The caller, for handlers that record who acted. */
	userId: string;
};

type SessionShape = { user: { id: string } };

/**
 * Resolves the session, loads the recast, and allows the creator, a workspace
 * owner/admin, or a platform admin. Roles are re-read per request so a change
 * takes effect immediately rather than waiting on session re-issue.
 */
export async function authorizeRecast(
	request: Request,
	recastId: string,
): Promise<AuthorizedRecast> {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	const db = getDb();
	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
			posterUrl: recast.posterUrl,
			sizeBytes: recast.sizeBytes,
			status: recast.status,
		})
		.from(recast)
		.where(eq(recast.id, recastId))
		.limit(1);
	if (!row) error(404, "Recast not found");

	const userId = session.user.id;
	if (row.ownerId !== userId) {
		const [[u], [membership]] = await Promise.all([
			db.select({ role: user.role }).from(user).where(eq(user.id, userId)).limit(1),
			db
				.select({ role: member.role })
				.from(member)
				.where(and(eq(member.organizationId, row.workspaceId), eq(member.userId, userId)))
				.limit(1),
		]);
		const allowed = canAccessRecast({
			recastOwnerId: row.ownerId,
			userId,
			workspaceRole: membership?.role,
			platformRole: u?.role,
		});
		if (!allowed) error(403, "Not allowed to modify this recast");
	}

	return { ...row, sizeBytes: Number(row.sizeBytes), userId };
}
