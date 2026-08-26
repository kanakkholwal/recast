import type { Cookies } from "@sveltejs/kit";
import { error } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { member, share, shareMember, user } from "$lib/db/schema";
import { resolveShareManage } from "$lib/share/manage";
import { constantTimeEquals, unlockCookieName, unlockToken } from "$lib/share/password";

/**
 * Shared visibility/password gate for share sub-resources (comments,
 * reactions). Mirrors the switch in `/api/share/[id]/video` so both honor
 * the same Google-Docs scopes; kept here so the engagement endpoints don't
 * duplicate (and drift from) that logic.
 *
 * Throws the appropriate HTTP error on deny (404 / 401 / 403 / 410) so call
 * sites can stay linear. On success returns the share row plus whether the
 * caller can MANAGE it (owner or global admin — gates moderation/delete).
 */

type SessionShape = {
	user: { id: string; email: string; name?: string | null; role?: string };
};

export type GatedShare = {
	slug: string;
	ownerId: string;
	visibility: string;
	commentsEnabled: boolean;
	canManage: boolean;
	/** Signed-in viewer's account id (null when anonymous) — server-only. */
	viewerId: string | null;
	/** Signed-in viewer's display name — used to server-stamp comment authorship
	 *  so a logged-in commenter can't spoof a name. Null when anonymous. */
	viewerName: string | null;
};

export async function gateShareAccess(
	slug: string,
	request: Request,
	cookies: Cookies,
): Promise<GatedShare> {
	const db = getDb();

	const [s] = await db
		.select({
			slug: share.slug,
			ownerId: share.ownerId,
			organizationId: share.organizationId,
			visibility: share.visibility,
			passwordHash: share.passwordHash,
			commentsEnabled: share.commentsEnabled,
			expiresAt: share.expiresAt,
		})
		.from(share)
		.where(eq(share.slug, slug))
		.limit(1);
	if (!s) error(404, "Share not found");

	if (s.expiresAt && s.expiresAt.getTime() < Date.now()) {
		// 410 Gone mirrors the video endpoint's treatment of expired shares.
		error(410, "This share link has expired");
	}

	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	const isOwner = session?.user?.id === s.ownerId;

	switch (s.visibility) {
		case "public":
			break;

		case "workspace":
		case "team": {
			if (!session?.user) error(401, "Sign in required");
			if (!s.organizationId) error(403, "Workspace share missing org");
			if (!isOwner) {
				const [m] = await db
					.select({ id: member.id })
					.from(member)
					.where(
						and(eq(member.userId, session.user.id), eq(member.organizationId, s.organizationId)),
					)
					.limit(1);
				if (!m) error(403, "Not a member of this workspace");
			}
			break;
		}

		case "selected": {
			if (!session?.user) error(401, "Sign in required");
			if (!isOwner) {
				const allowed = await db
					.select({ id: shareMember.id })
					.from(shareMember)
					.where(and(eq(shareMember.shareSlug, s.slug), eq(shareMember.email, session.user.email)))
					.limit(1);
				if (allowed.length === 0) error(403, "Not on the access list");
			}
			break;
		}

		case "private": {
			if (!session?.user) error(401, "Sign in required");
			if (!isOwner) {
				const [u] = await db
					.select({ role: user.role })
					.from(user)
					.where(eq(user.id, session.user.id))
					.limit(1);
				if (u?.role !== "admin") error(403, "Private share");
			}
			break;
		}

		default:
			error(500, "Unknown visibility");
	}

	// Password gate (orthogonal to visibility) — same unlock cookie the
	// page loader and video endpoint mint.
	if (s.passwordHash) {
		const got = cookies.get(unlockCookieName(s.slug));
		const expected = await unlockToken(s.slug);
		if (!got || !constantTimeEquals(got, expected)) {
			error(401, "Password required");
		}
	}

	// Manage = share owner, an owner/admin of the recast's workspace, or a
	// global admin — see `resolveShareManage` (shared with the page loader and
	// the settings/access endpoints so all four agree on who can moderate).
	let canManage = false;
	if (session?.user) {
		const mng = await resolveShareManage(s.slug, session.user.id);
		canManage = mng?.canManage ?? false;
	}

	return {
		slug: s.slug,
		ownerId: s.ownerId,
		visibility: s.visibility,
		commentsEnabled: s.commentsEnabled,
		canManage,
		viewerId: session?.user?.id ?? null,
		viewerName: session?.user?.name ?? null,
	};
}
