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
import { deliveryState, getQuotaSnapshot, storagePctUsed } from "$lib/storage/quota";
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
		.innerJoin(organizationTable, eq(memberTable.organizationId, organizationTable.id))
		.where(eq(memberTable.userId, session.user.id))
		.orderBy(desc(memberTable.createdAt));

	// Streamed so the shell and sidebar render immediately; consumed downstream by the invite banners.
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
		.innerJoin(organizationTable, eq(invitationTable.organizationId, organizationTable.id))
		.where(
			and(eq(invitationTable.email, session.user.email), eq(invitationTable.status, "pending")),
		);

	// No memberships means onboarding; /onboarding/team sits outside /dashboard, so this can't loop.
	if (memberships.length === 0) {
		redirect(303, "/onboarding/team");
	}

	let activeOrganizationId = session.session?.activeOrganizationId ?? null;
	let activeMembership = memberships.find((m) => m.organizationId === activeOrganizationId);
	if (!activeMembership) {
		// The session lost its activeOrganizationId, so restore the default workspace or the most recent membership.
		activeMembership =
			memberships.find((m) => m.organizationId === userPrefs?.defaultWorkspaceId) ?? memberships[0];
		activeOrganizationId = activeMembership.organizationId;
		try {
			await getAuth().api.setActiveOrganization({
				headers: request.headers,
				body: { organizationId: activeOrganizationId },
			});
		} catch (err) {
			console.error("[dashboard] setActiveOrganization failed", err);
		}
	}

	// Coerce Infinity to null so the quota snapshot survives `JSON.stringify`, which drops it.
	const snap = await getQuotaSnapshot(activeMembership.organizationId);
	const finite = (n: number): number | null => (Number.isFinite(n) ? n : null);
	const delivery = snap ? deliveryState(snap) : null;
	const quota =
		snap && delivery
			? {
					plan: snap.plan,
					usage: {
						storageBytes: snap.usage.storageBytes,
						activeRecastsCount: snap.usage.activeRecastsCount,
						archivedRecastsCount: snap.usage.archivedRecastsCount,
						membersCount: snap.usage.membersCount,
						// From deliveryState, so a rolled-over month reads as 0 here too.
						deliveryBytesThisMonth: delivery.usedBytes,
					},
					limits: {
						storageBytes: finite(snap.limits.storageBytes),
						activeRecasts: finite(snap.limits.activeRecasts),
						members: finite(snap.limits.members),
						maxDurationSec: finite(snap.limits.maxDurationSec),
						playbackMaxHeight: snap.limits.playbackMaxHeight,
						deliveryBytesPerMonth: finite(snap.limits.deliveryBytesPerMonth),
					},
					storagePctUsed: storagePctUsed(snap),
					delivery: {
						usedBytes: delivery.usedBytes,
						capBytes: finite(delivery.capBytes),
						ratio: delivery.ratio,
						exceeded: delivery.exceeded,
						warn: delivery.warn,
					},
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
