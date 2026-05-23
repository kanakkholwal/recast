import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { share, user } from "$lib/db/schema";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string; role?: string; activeOrganizationId?: string | null } };

const VALID = new Set(["public", "team", "private"] as const);
type Visibility = "public" | "team" | "private";

/**
 * PATCH /api/share/[id]/access
 *
 * Owner-or-admin endpoint to change a share's visibility. Body:
 *   { visibility: "public" | "team" | "private", organizationId?: string }
 *
 * Rules:
 *   - Visibility must be one of the three enum values.
 *   - `team` requires an `organizationId`. We pull the caller's active org
 *     from the session as the default, but accept an explicit value so a
 *     future "share to a specific team" picker can override.
 *   - Only the share owner or a global admin may change settings.
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
	if (!VALID.has(visibility as Visibility)) {
		error(400, "Invalid visibility value");
	}

	const db = getDb();

	const [row] = await db
		.select({ ownerId: share.ownerId })
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);

	if (!row) error(404, "Share not found");

	// Authorize: owner OR global admin. We re-read the user row instead
	// of trusting the session's `role` so a role change takes effect on
	// the next request without waiting for the session to re-issue.
	const isOwner = row.ownerId === session.user.id;
	let isAdmin = false;
	if (!isOwner) {
		const [u] = await db
			.select({ role: user.role })
			.from(user)
			.where(eq(user.id, session.user.id))
			.limit(1);
		isAdmin = u?.role === "admin";
	}
	if (!isOwner && !isAdmin) error(403, "Not allowed to change this share");

	const next = visibility as Visibility;

	let organizationId: string | null = null;
	if (next === "team") {
		const explicit = typeof body.organizationId === "string" ? body.organizationId : null;
		organizationId = explicit ?? session.user.activeOrganizationId ?? null;
		if (!organizationId) {
			error(400, "Team visibility requires an active team");
		}
	}

	await db
		.update(share)
		.set({ visibility: next, organizationId })
		.where(eq(share.slug, params.id));

	return json({ ok: true, visibility: next, organizationId });
};
