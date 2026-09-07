import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { member, recast, share, shareMember, user } from "$lib/db/schema";
import { normalizeEmail } from "$lib/share/grant";

/**
 * Share access resolution.
 *
 * The share page needs three things in one trip:
 *   1. The recast row (for the player)
 *   2. Whether the viewer can VIEW it (gates the player vs a denial card)
 *   3. Whether the viewer can MANAGE it (owner OR global admin —
 *      surfaces the "Who can view" control in the Share dropdown)
 *
 * Same-team viewers who are denied still get a contact email back so the
 * UI can render a "request access" affordance instead of a dead end.
 */

/**
 * `team` is the legacy enum value preserved for backwards compatibility with
 * old share rows. New writes should use `workspace`; consumers treat the
 * two identically.
 */
export type ShareVisibility = "public" | "workspace" | "team" | "selected" | "private";

export type ResolvedShare =
	| {
			ok: true;
			recast: {
				id: string;
				title: string;
				description: string;
				src: string;
				poster: string | null;
				/** WebVTT captions track (signed URL), or null when none. */
				captions: string | null;
				durationSec: number;
				/** Intrinsic pixel dimensions of the source video, when known.
				 *  The player reserves this exact aspect ratio before metadata
				 *  loads so the hero never shifts. Null on legacy rows. */
				width: number | null;
				height: number | null;
				sharedBy: string;
				sharedAt: number;
			};
			share: {
				slug: string;
				visibility: ShareVisibility;
				organizationId: string | null;
				ctaLabel: string | null;
				ctaUrl: string | null;
				commentsEnabled: boolean;
				viewsCount: number;
				/** Owner opt-in to search indexing (public shares only). */
				searchable: boolean;
			};
			canManage: boolean;
	  }
	| {
			ok: false;
			reason: "not-found" | "denied";
			visibility?: ShareVisibility;
			ownerEmail?: string;
			sameTeam?: boolean;
	  };

type Viewer = {
	id: string;
	email: string;
	role: string;
	/** orgId → the viewer's `member.role` in that org ("owner"|"admin"|"member").
	 *  Keys double as the set of orgs the viewer belongs to. */
	memberships: Map<string, string>;
} | null;

export async function loadViewer(userId: string | null | undefined): Promise<Viewer> {
	if (!userId) return null;
	const db = getDb();
	const [u] = await db
		.select({ id: user.id, email: user.email, role: user.role })
		.from(user)
		.where(eq(user.id, userId))
		.limit(1);
	if (!u) return null;
	const memberships = await db
		.select({ organizationId: member.organizationId, role: member.role })
		.from(member)
		.where(eq(member.userId, userId));
	return {
		id: u.id,
		email: u.email,
		role: u.role,
		memberships: new Map(memberships.map((m) => [m.organizationId, m.role])),
	};
}

export async function resolveShareAccess(
	slug: string,
	viewer: Viewer,
	/**
	 * Email certified by a valid grant cookie (see `$lib/share/grant`), for
	 * account-less `selected`-share invitees. Null for everyone else. The
	 * caller derives + verifies the cookie; this function still re-checks the
	 * email against the allowlist, so a stale grant can't outlive a removal.
	 */
	grantedEmail: string | null = null,
): Promise<ResolvedShare> {
	const db = getDb();
	const rows = await db
		.select({
			slug: share.slug,
			visibility: share.visibility,
			organizationId: share.organizationId,
			ctaLabel: share.ctaLabel,
			ctaUrl: share.ctaUrl,
			commentsEnabled: share.commentsEnabled,
			viewsCount: share.viewsCount,
			searchable: share.searchable,
			ownerId: share.ownerId,
			ownerEmail: user.email,
			ownerName: user.name,
			recastId: recast.id,
			workspaceId: recast.workspaceId,
			title: recast.title,
			description: recast.description,
			videoUrl: recast.videoUrl,
			posterUrl: recast.posterUrl,
			captionsUrl: recast.captionsUrl,
			durationSec: recast.durationSec,
			width: recast.width,
			height: recast.height,
			createdAt: recast.createdAt,
		})
		.from(share)
		.innerJoin(recast, eq(share.recastId, recast.id))
		.innerJoin(user, eq(share.ownerId, user.id))
		.where(and(eq(share.slug, slug)))
		.limit(1);
	const row = rows[0];
	if (!row) return { ok: false, reason: "not-found" };

	const isOwner = viewer?.id === row.ownerId;
	const isAdmin = viewer?.role === "admin";
	const inOrg = row.organizationId != null && viewer?.memberships.has(row.organizationId) === true;
	// Workspace owners and admins manage every share in it, not just the ones they created. See `resolveShareManage`.
	const workspaceRole = viewer?.memberships.get(row.workspaceId);
	const isWorkspaceManager = workspaceRole === "owner" || workspaceRole === "admin";

	// `selected` adds a per-share allowlist; a signed-in email or a grant cookie both re-check `share_member`, so removal revokes at once.
	let onAllowlist = false;
	if (row.visibility === "selected" && !isOwner && !isAdmin) {
		const candidates = [viewer?.email, grantedEmail]
			.filter((e): e is string => Boolean(e))
			.map(normalizeEmail);
		if (candidates.length > 0) {
			const members = await db
				.select({ email: shareMember.email })
				.from(shareMember)
				.where(eq(shareMember.shareSlug, slug));
			const allow = new Set(members.map((m) => normalizeEmail(m.email)));
			onAllowlist = candidates.some((e) => allow.has(e));
		}
	}

	// `team` is the legacy alias for `workspace`. Manage implies view, or admins would reach manage endpoints yet be denied the share.
	const canView =
		row.visibility === "public" ||
		isOwner ||
		isAdmin ||
		isWorkspaceManager ||
		((row.visibility === "team" || row.visibility === "workspace") && inOrg) ||
		(row.visibility === "selected" && onAllowlist);

	const canManage = isOwner || isAdmin || isWorkspaceManager;

	if (!canView) {
		return {
			ok: false,
			reason: "denied",
			visibility: row.visibility,
			ownerEmail: row.ownerEmail,
			// Signed in, same org, still denied: that is a private share, and the request-access CTA only makes sense there.
			sameTeam: inOrg,
		};
	}

	return {
		ok: true,
		recast: {
			id: row.recastId,
			title: row.title,
			description: row.description ?? "",
			src: row.videoUrl,
			poster: row.posterUrl,
			captions: row.captionsUrl,
			durationSec: row.durationSec,
			width: row.width,
			height: row.height,
			sharedBy: row.ownerName,
			sharedAt: row.createdAt.getTime(),
		},
		share: {
			slug: row.slug,
			visibility: row.visibility,
			organizationId: row.organizationId,
			ctaLabel: row.ctaLabel,
			ctaUrl: row.ctaUrl,
			commentsEnabled: row.commentsEnabled,
			viewsCount: row.viewsCount,
			searchable: row.searchable,
		},
		canManage,
	};
}
