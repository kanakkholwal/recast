import { error, json, type RequestHandler } from "@sveltejs/kit";
import { and, count, eq, gt, isNull, or, sum } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { limitsFor, planOf } from "$lib/billing/plans";
import { getDb } from "$lib/db";
import {
	member as memberTable,
	organization as organizationTable,
	recast as recastTable,
	share as shareTable,
	subscription as subscriptionTable,
	user as userTable,
} from "$lib/db/schema";

type SessionShape = {
	user: {
		id: string;
		email: string;
		name?: string | null;
		image?: string | null;
		activeOrganizationId?: string | null;
	};
};

/**
 * Desktop "Sign in to Cloud" profile endpoint.
 *
 * Returns enough data for the desktop's Settings → Cloud signed-in card to
 * render a real user profile (avatar, plan badge, usage stats) without the
 * frontend needing N parallel calls. Authenticated via the bearer plugin —
 * the desktop passes `Authorization: Bearer <session.token>`.
 *
 * Why this endpoint (vs. just /api/auth/get-session): get-session only
 * returns the user row. We also need the user's plan (from `subscription`),
 * recordings count + storage usage (sum from `recast`), and active-share
 * count (from `share`). One round-trip is cheaper than three.
 */
export const GET: RequestHandler = async ({ request }) => {
	const auth = getAuth();
	const session = (await auth.api
		.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session?.user?.id) throw error(401, "unauthorized");

	const db = getDb();
	const userId = session.user.id;

	// Run the aggregate queries in parallel — they don't depend on each
	// other and each is cheap (single-table indexed scan / counter read).
	const [userRow, subRows, recastAgg, shareAgg, memberships, workspaceRecastCounts] =
		await Promise.all([
			db
				.select({
					email: userTable.email,
					name: userTable.name,
					image: userTable.image,
					defaultWorkspaceId: userTable.defaultWorkspaceId,
					createdAt: userTable.createdAt,
				})
				.from(userTable)
				.where(eq(userTable.id, userId))
				.limit(1)
				.then((rows) => rows[0] ?? null),
			// One user can bill several workspaces, so fetch all and pick the row
			// for the default upload target below.
			db
				.select({
					organizationId: subscriptionTable.organizationId,
					plan: subscriptionTable.plan,
					status: subscriptionTable.status,
					currentPeriodEnd: subscriptionTable.currentPeriodEnd,
					cancelAtPeriodEnd: subscriptionTable.cancelAtPeriodEnd,
				})
				.from(subscriptionTable)
				.where(eq(subscriptionTable.userId, userId)),
			db
				.select({
					recordings: count(),
					// Drizzle's `sum` returns string | null on PG for bigint columns;
					// coerce after the fetch.
					storage: sum(recastTable.sizeBytes),
				})
				.from(recastTable)
				.where(and(eq(recastTable.ownerId, userId), isNull(recastTable.deletedAt)))
				.then((rows) => rows[0] ?? { recordings: 0, storage: "0" }),
			db
				.select({ active: count() })
				.from(shareTable)
				.where(
					and(
						eq(shareTable.ownerId, userId),
						// "Active" = no expiry OR not yet expired.
						or(isNull(shareTable.expiresAt), gt(shareTable.expiresAt, new Date())),
					),
				)
				.then((rows) => rows[0] ?? { active: 0 }),
			// Workspaces the user belongs to — the desktop needs an explicit
			// workspaceId for /api/uploads/init (its device session may not
			// carry an activeOrganizationId).
			db
				.select({
					id: organizationTable.id,
					name: organizationTable.name,
					role: memberTable.role,
					plan: organizationTable.plan,
				})
				.from(memberTable)
				.innerJoin(organizationTable, eq(memberTable.organizationId, organizationTable.id))
				.where(eq(memberTable.userId, userId)),
			// Live (non-deleted) recast count per workspace the user belongs to.
			// The inner join to `member` both scopes to the user's workspaces and,
			// because each recast matches exactly one of the user's membership rows,
			// keeps the count equal to the workspace's total recast count.
			db
				.select({
					workspaceId: recastTable.workspaceId,
					count: count(),
				})
				.from(recastTable)
				.innerJoin(
					memberTable,
					and(
						eq(memberTable.organizationId, recastTable.workspaceId),
						eq(memberTable.userId, userId),
					),
				)
				.where(isNull(recastTable.deletedAt))
				.groupBy(recastTable.workspaceId),
		]);

	if (!userRow) throw error(404, "user_not_found");

	// Default upload target: the user's saved default workspace if still valid,
	// else the session's active org, else their first workspace.
	const recastCountByWorkspace = new Map(
		workspaceRecastCounts.map((r) => [r.workspaceId, Number(r.count) || 0]),
	);
	const workspaces = memberships.map((m) => ({
		id: m.id,
		name: m.name,
		role: m.role,
		// `organization.plan` is "free" | "pro" | "enterprise" (admin-managed).
		plan: m.plan ?? "free",
		recastsCount: recastCountByWorkspace.get(m.id) ?? 0,
	}));
	const activeId = session.user.activeOrganizationId ?? null;
	const defaultWorkspaceId =
		(userRow.defaultWorkspaceId && workspaces.some((w) => w.id === userRow.defaultWorkspaceId)
			? userRow.defaultWorkspaceId
			: activeId && workspaces.some((w) => w.id === activeId)
				? activeId
				: workspaces[0]?.id) ?? null;

	// Entitlements are workspace-scoped, so the reported plan is the default
	// upload target's — not the user's, who may own workspaces on other plans.
	const defaultWorkspace = workspaces.find((w) => w.id === defaultWorkspaceId);
	const plan = planOf(defaultWorkspace?.plan);
	const sharesLimit = limitsFor(plan.id).activeRecasts;
	const subRow = subRows.find((s) => s.organizationId === defaultWorkspaceId) ?? null;

	return json({
		user: {
			email: userRow.email,
			name: userRow.name ?? null,
			image: userRow.image ?? null,
			memberSince: userRow.createdAt?.toISOString() ?? null,
		},
		plan: {
			id: plan.id,
			name: plan.name,
			status: subRow?.status ?? "active",
			currentPeriodEnd: subRow?.currentPeriodEnd?.toISOString() ?? null,
			cancelAtPeriodEnd: subRow?.cancelAtPeriodEnd ?? false,
		},
		usage: {
			recordings: Number(recastAgg.recordings) || 0,
			storageBytes: Number(recastAgg.storage ?? 0) || 0,
			activeShares: Number(shareAgg.active) || 0,
			sharesLimit,
		},
		workspaces,
		defaultWorkspaceId,
	});
};
