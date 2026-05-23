import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { member, recast, share, user } from "$lib/db/schema";

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

export type ResolvedShare =
	| {
			ok: true;
			recast: {
				id: string;
				title: string;
				description: string;
				src: string;
				poster: string | null;
				durationSec: number;
				sharedBy: string;
				sharedAt: number;
			};
			share: {
				slug: string;
				visibility: "public" | "team" | "private";
				organizationId: string | null;
			};
			canManage: boolean;
	  }
	| {
			ok: false;
			reason: "not-found" | "denied";
			visibility?: "public" | "team" | "private";
			ownerEmail?: string;
			sameTeam?: boolean;
	  };

type Viewer = {
	id: string;
	role: string;
	memberOrgIds: Set<string>;
} | null;

export async function loadViewer(userId: string | null | undefined): Promise<Viewer> {
	if (!userId) return null;
	const db = getDb();
	const [u] = await db
		.select({ id: user.id, role: user.role })
		.from(user)
		.where(eq(user.id, userId))
		.limit(1);
	if (!u) return null;
	const memberships = await db
		.select({ organizationId: member.organizationId })
		.from(member)
		.where(eq(member.userId, userId));
	return {
		id: u.id,
		role: u.role,
		memberOrgIds: new Set(memberships.map((m) => m.organizationId)),
	};
}

export async function resolveShareAccess(
	slug: string,
	viewer: Viewer,
): Promise<ResolvedShare> {
	const db = getDb();
	const rows = await db
		.select({
			slug: share.slug,
			visibility: share.visibility,
			organizationId: share.organizationId,
			ownerId: share.ownerId,
			ownerEmail: user.email,
			ownerName: user.name,
			recastId: recast.id,
			title: recast.title,
			videoUrl: recast.videoUrl,
			posterUrl: recast.posterUrl,
			durationSec: recast.durationSec,
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
	const inOrg =
		row.organizationId != null &&
		viewer?.memberOrgIds.has(row.organizationId) === true;

	const canView =
		row.visibility === "public" ||
		isOwner ||
		isAdmin ||
		(row.visibility === "team" && inOrg);

	const canManage = isOwner || isAdmin;

	if (!canView) {
		return {
			ok: false,
			reason: "denied",
			visibility: row.visibility,
			ownerEmail: row.ownerEmail,
			// Same-team viewer: signed in, in the same org, but denied. This
			// happens for `private` shares — the "request access" CTA only
			// makes sense in that case.
			sameTeam: inOrg,
		};
	}

	return {
		ok: true,
		recast: {
			id: row.recastId,
			title: row.title,
			description: "",
			src: row.videoUrl,
			poster: row.posterUrl,
			durationSec: row.durationSec,
			sharedBy: row.ownerName,
			sharedAt: row.createdAt.getTime(),
		},
		share: {
			slug: row.slug,
			visibility: row.visibility,
			organizationId: row.organizationId,
		},
		canManage,
	};
}
