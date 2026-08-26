import { error } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { member, user } from "$lib/db/schema";

/**
 * Shared auth helpers for workspace-scoped resources (folders, tags, recast
 * mutations). Centralizes the "signed-in + member of this workspace" check so
 * the new endpoints don't each re-implement (and drift from) it.
 */

export type SessionUser = {
	id: string;
	email: string;
	role?: string;
	activeOrganizationId?: string | null;
};

/** Resolve the signed-in user or throw 401. */
export async function requireUser(request: Request): Promise<SessionUser> {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as { user: SessionUser } | null;
	if (!session?.user) error(401, "Sign in required");
	return session.user;
}

/**
 * Assert the user is a member of `workspaceId` (or a global admin). Throws
 * 403 otherwise. Owner-only folders/tags still live inside a workspace, so
 * membership is the gate; per-folder permissions are a later concern.
 */
export async function assertWorkspaceMember(userId: string, workspaceId: string): Promise<void> {
	const db = getDb();
	const [m] = await db
		.select({ id: member.id })
		.from(member)
		.where(and(eq(member.userId, userId), eq(member.organizationId, workspaceId)))
		.limit(1);
	if (m) return;
	// Global admins (re-read so a role change takes effect immediately) bypass.
	const [u] = await db.select({ role: user.role }).from(user).where(eq(user.id, userId)).limit(1);
	if (u?.role === "admin") return;
	error(403, "Not a member of this workspace");
}
