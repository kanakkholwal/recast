import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { share } from "$lib/db/schema";
import { resolveShareManage } from "$lib/share/manage";
import { assertWorkspaceMember } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string; role?: string; activeOrganizationId?: string | null } };

// `team` is the legacy alias for `workspace`. `selected` is excluded: an allowlist needs invitees this endpoint can't take.
const VALID = new Set(["public", "workspace", "team", "private"] as const);
type Visibility = "public" | "workspace" | "team" | "private";

/**
 * PATCH /api/share/[id]/access
 *
 * Change a share's visibility. Body: { visibility, organizationId? }.
 *
 * Rules:
 *   - Visibility ∈ {public, workspace|team, private}. `selected` is rejected
 *     (use the share dialog to pick people on a new link).
 *   - workspace/team binds the share to a workspace. It defaults to the
 *     recast's own workspace; an explicit `organizationId` is allowed ONLY if
 *     the caller is a member of it (or a global admin) — otherwise a share
 *     owner could expose the recast to a workspace they're not in.
 *   - Manageable by the share owner, an owner/admin of the recast's workspace,
 *     or a global admin (see `resolveShareManage`).
 */
export const PATCH: RequestHandler = async ({ params, request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session?.user) error(401, "Sign in required");

	let body: { visibility?: unknown; organizationId?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const visibility = typeof body.visibility === "string" ? body.visibility : "";
	if (visibility === "selected") {
		error(400, "To share with specific people, create a new link from the share dialog");
	}
	if (!VALID.has(visibility as Visibility)) {
		error(400, "Invalid visibility value");
	}
	// Normalize the legacy `team` alias so new writes stop persisting the deprecated value.
	const next: Visibility = visibility === "team" ? "workspace" : (visibility as Visibility);

	// Authorize against the share + its recast's workspace in one shared check.
	const manage = await resolveShareManage(params.id, session.user.id);
	if (!manage) error(404, "Share not found");
	if (!manage.canManage) error(403, "Not allowed to change this share");

	let organizationId: string | null = null;
	if (next === "workspace") {
		const explicit = typeof body.organizationId === "string" ? body.organizationId : null;
		// Default to the recast's own workspace; an explicit override must be one the caller belongs to, and admins pass while others 403.
		organizationId = explicit ?? manage.workspaceId ?? session.user.activeOrganizationId ?? null;
		if (!organizationId) {
			error(400, "Team visibility requires a workspace");
		}
		if (organizationId !== manage.workspaceId) {
			await assertWorkspaceMember(session.user.id, organizationId);
		}
	}

	const db = getDb();
	await db.update(share).set({ visibility: next, organizationId }).where(eq(share.slug, params.id));

	return json({ ok: true, visibility: next, organizationId });
};
