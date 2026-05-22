import { error, redirect } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import {
	invitation as invitationTable,
	organization as organizationTable,
} from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

type SessionShape = { user: { id: string; email: string } };

/**
 * Lands here from the invitation email. We:
 *
 *   1. Validate the id and fetch the invite (404 if unknown).
 *   2. If the user isn't signed in → /login?next=…, preserving the id so
 *      this page reruns post-sign-in.
 *   3. If the signed-in email doesn't match the invitation email, show a
 *      "wrong account" message — don't auto-accept against the wrong row.
 *   4. Otherwise hand the id to the page so the client can call
 *      `authClient.organization.acceptInvitation({ invitationId })` (or
 *      reject) — keeps cookies updated for `setActiveOrganization`.
 */
export const load: PageServerLoad = async ({ url, request }) => {
	const id = url.searchParams.get("id");
	if (!id) error(404, "Invitation not found");

	const db = getDb();
	const [inv] = await db
		.select({
			id: invitationTable.id,
			email: invitationTable.email,
			role: invitationTable.role,
			status: invitationTable.status,
			expiresAt: invitationTable.expiresAt,
			organizationId: invitationTable.organizationId,
			orgName: organizationTable.name,
		})
		.from(invitationTable)
		.innerJoin(
			organizationTable,
			eq(invitationTable.organizationId, organizationTable.id),
		)
		.where(eq(invitationTable.id, id))
		.limit(1);

	if (!inv) error(404, "Invitation not found");

	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session) {
		const next = encodeURIComponent(`/accept-invitation?id=${id}`);
		redirect(303, `/login?next=${next}`);
	}

	const emailMatches =
		session.user.email.toLowerCase() === inv.email.toLowerCase();
	const expired = inv.expiresAt && new Date(inv.expiresAt).getTime() < Date.now();

	return {
		invite: {
			id: inv.id,
			email: inv.email,
			role: inv.role,
			status: inv.status,
			orgName: inv.orgName,
			expired: Boolean(expired),
		},
		viewer: { email: session.user.email, emailMatches },
	};
};
