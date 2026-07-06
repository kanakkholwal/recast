import { redirect } from "@sveltejs/kit";
import { and, desc, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import {
	invitation as invitationTable,
	member as memberTable,
	organization as organizationTable,
	user as userTable,
} from "$lib/db/schema";
import { getQuotaSnapshot, storagePctUsed } from "$lib/storage/quota";
import type { LayoutServerLoad } from "./$types";

type SessionUser = {
	id: string;
	name?: string | null;
	email: string;
	role?: string | null;
	emailVerified?: boolean | null;
};
type SessionShape = {
	user: SessionUser;
	session: { activeOrganizationId?: string | null };
};

/**
 * Dashboard auth + team gate.
 *
 *   1. No session → /login?next=…
 *   2. Not email-verified → /verify-email (full dashboard is gated; users
 *      can still see the marketing site and verification page itself).
 *      Magic-link sign-in auto-verifies, and invitees are pre-created with
 *      `emailVerified: true`, so this only catches password signups that
 *      haven't clicked the confirmation link yet.
 *   3. No teams at all → /onboarding/team (create one or accept an invite)
 *   4. No active team set but has memberships → auto-set the most recent
 *      one and rerun. Avoids forcing onboarding on users whose session
 *      simply lost activeOrganizationId (logged in fresh, etc).
 */
export const load: LayoutServerLoad = async ({ request, url }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session) {
		redirect(303, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	}

	if (!session.user.emailVerified) {
		redirect(303, "/verify-email");
	}

	const db = getDb();
	const [userPrefs] = await db
		.select({ defaultWorkspaceId: userTable.defaultWorkspaceId })
		.from(userTable)
		.where(eq(userTable.id, session.user.id))
		.limit(1);
	const memberships = await db
		.select({
			organizationId: memberTable.organizationId,
			role: memberTable.role,
			name: organizationTable.name,
			slug: organizationTable.slug,
			plan: organizationTable.plan,
		})
		.from(memberTable)
		.innerJoin(
			organizationTable,
			eq(memberTable.organizationId, organizationTable.id),
		)
		.where(eq(memberTable.userId, session.user.id))
		.orderBy(desc(memberTable.createdAt));

	// Pending invitations addressed to this email — streamed so the dashboard
	// shell + sidebar render immediately. Consumed downstream (e.g. invite
	// banners); the query keeps running but no longer blocks initial paint.
	const pendingInvites = db
		.select({
			id: invitationTable.id,
			email: invitationTable.email,
			organizationId: invitationTable.organizationId,
			orgName: organizationTable.name,
			role: invitationTable.role,
			status: invitationTable.status,
			expiresAt: invitationTable.expiresAt,
		})
		.from(invitationTable)
		.innerJoin(
			organizationTable,
			eq(invitationTable.organizationId, organizationTable.id),
		)
		.where(
			and(
				eq(invitationTable.email, session.user.email),
				eq(invitationTable.status, "pending"),
			),
		);

	// No memberships → onboarding. /onboarding/team is OUTSIDE /dashboard so
	// this redirect doesn't loop.
	if (memberships.length === 0) {
		redirect(303, "/onboarding/team");
	}

	let activeOrganizationId = session.session?.activeOrganizationId ?? null;
	if (!activeOrganizationId || !memberships.find((m) => m.organizationId === activeOrganizationId)) {
		// Session lost activeOrganizationId (or it points at a team the user
		// no longer belongs to). Restore from the user's default workspace when
		// possible, otherwise pick the most recent membership.
		const fallback =
			memberships.find((m) => m.organizationId === userPrefs?.defaultWorkspaceId) ??
			memberships[0]!;
		activeOrganizationId = fallback.organizationId;
		try {
			await getAuth().api.setActiveOrganization({
				headers: request.headers,
				body: { organizationId: fallback.organizationId },
			});
		} catch (err) {
			console.error("[dashboard] setActiveOrganization failed", err);
		}
	}

	const activeMembership = memberships.find(
		(m) => m.organizationId === activeOrganizationId,
	)!;

	// Live quota snapshot for the active workspace — feeds the sidebar
	// usage meter, the upload-button enable state, and the
	// transparency surface on settings/billing. Coerce Infinity to null
	// so the JSON payload survives `JSON.stringify` (which drops it).
	const snap = await getQuotaSnapshot(activeMembership.organizationId);
	const finite = (n: number): number | null => (Number.isFinite(n) ? n : null);
	const quota = snap
		? {
				plan: snap.plan,
				usage: {
					storageBytes: snap.usage.storageBytes,
					activeRecastsCount: snap.usage.activeRecastsCount,
					archivedRecastsCount: snap.usage.archivedRecastsCount,
					membersCount: snap.usage.membersCount,
				},
				limits: {
					storageBytes: finite(snap.limits.storageBytes),
					activeRecasts: finite(snap.limits.activeRecasts),
					members: finite(snap.limits.members),
					maxDurationSec: finite(snap.limits.maxDurationSec),
					playbackMaxHeight: snap.limits.playbackMaxHeight,
				},
				storagePctUsed: storagePctUsed(snap),
			}
		: null;

	return {
		user: {
			id: session.user.id,
			name: session.user.name ?? "",
			email: session.user.email,
			role: session.user.role ?? "user",
			emailVerified: Boolean(session.user.emailVerified),
			defaultWorkspaceId: userPrefs?.defaultWorkspaceId ?? null,
		},
		memberships,
		pendingInvites,
		activeOrganization: {
			id: activeMembership.organizationId,
			name: activeMembership.name,
			slug: activeMembership.slug,
			plan: activeMembership.plan,
			role: activeMembership.role,
			isDefault: activeMembership.organizationId === userPrefs?.defaultWorkspaceId,
		},
		quota,
	};
};
