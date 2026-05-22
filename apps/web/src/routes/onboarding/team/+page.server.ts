import { redirect } from "@sveltejs/kit";
import { and, count, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import {
	invitation as invitationTable,
	member as memberTable,
	organization as organizationTable,
	USER_TEAM_OWNERSHIP_CAPS,
} from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

type SessionShape = { user: { id: string; email: string; name?: string | null } };

/**
 * Onboarding gate — only reachable when a signed-in user has zero team
 * memberships. If they already belong to a team we bounce to /dashboard.
 */
export const load: PageServerLoad = async ({ request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session) redirect(303, "/login?next=/onboarding/team");

	const db = getDb();
	const [{ c }] = await db
		.select({ c: count() })
		.from(memberTable)
		.where(eq(memberTable.userId, session.user.id));

	if (c > 0) redirect(303, "/dashboard");

	const invites = await db
		.select({
			id: invitationTable.id,
			orgName: organizationTable.name,
			role: invitationTable.role,
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

	return {
		invites,
		caps: USER_TEAM_OWNERSHIP_CAPS,
		user: { name: session.user.name ?? "", email: session.user.email },
	};
};
