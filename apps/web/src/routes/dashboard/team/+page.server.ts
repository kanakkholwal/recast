import { error, fail, redirect } from "@sveltejs/kit";
import { and, desc, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import {
	TEAM_PLAN_MEMBER_CAPS,
	invitation as invitationTable,
	member as memberTable,
	organization as organizationTable,
	user as userTable,
} from "$lib/db/schema";
import type { Actions, PageServerLoad } from "./$types";

type SessionShape = {
	user: { id: string; email: string };
	session: { activeOrganizationId?: string | null };
};

async function loadActiveOrg(headers: Headers) {
	const session = (await getAuth()
		.api.getSession({ headers })
		.catch(() => null)) as SessionShape | null;
	if (!session) redirect(303, "/login");
	const orgId = session.session?.activeOrganizationId;
	if (!orgId) redirect(303, "/onboarding/team");

	const db = getDb();
	const [me] = await db
		.select({ role: memberTable.role })
		.from(memberTable)
		.where(
			and(
				eq(memberTable.organizationId, orgId),
				eq(memberTable.userId, session.user.id),
			),
		)
		.limit(1);
	if (!me) error(403, "Not a member of this team");

	return { userId: session.user.id, orgId, myRole: me.role };
}

/** Manager-only gate. `loadActiveOrg` only proves membership; mutating
 *  actions also need owner/admin role. Better Auth's `auth.api.*` enforces
 *  this internally, but checking here lets us return a clean SvelteKit
 *  form failure instead of a thrown APIError. */
function isManager(role: string): boolean {
	return role === "owner" || role === "admin";
}

export const load: PageServerLoad = async ({ request }) => {
	const { userId, orgId, myRole } = await loadActiveOrg(request.headers);
	const db = getDb();

	const [org] = await db
		.select()
		.from(organizationTable)
		.where(eq(organizationTable.id, orgId))
		.limit(1);
	if (!org) error(404, "Team not found");

	// Streamed — the team header (name, plan, seat cap) renders immediately
	// while the member list + pending invites fill in.
	const members = db
		.select({
			id: memberTable.id,
			role: memberTable.role,
			createdAt: memberTable.createdAt,
			userId: memberTable.userId,
			email: userTable.email,
			name: userTable.name,
		})
		.from(memberTable)
		.innerJoin(userTable, eq(memberTable.userId, userTable.id))
		.where(eq(memberTable.organizationId, orgId))
		.orderBy(desc(memberTable.createdAt));

	const invites = db
		.select()
		.from(invitationTable)
		.where(
			and(
				eq(invitationTable.organizationId, orgId),
				eq(invitationTable.status, "pending"),
			),
		)
		.orderBy(desc(invitationTable.createdAt));

	const memberCap = TEAM_PLAN_MEMBER_CAPS[org.plan] ?? TEAM_PLAN_MEMBER_CAPS.free!;

	return {
		org,
		members,
		invites,
		viewer: { userId, role: myRole },
		caps: { members: memberCap },
	};
};

export const actions: Actions = {
	updateProfile: async ({ request }) => {
		const { orgId, myRole } = await loadActiveOrg(request.headers);
		if (myRole !== "owner") {
			return fail(403, { error: "Only the owner can edit team details." });
		}
		const fd = await request.formData();
		const name = String(fd.get("name") ?? "").trim();
		const slug = String(fd.get("slug") ?? "").trim().toLowerCase();
		const logoRaw = String(fd.get("logo") ?? "").trim();
		const logo = logoRaw.length === 0 ? null : logoRaw;

		if (!name) return fail(400, { error: "Name is required" });
		if (!/^[a-z0-9][a-z0-9-]{1,62}[a-z0-9]$/.test(slug)) {
			return fail(400, {
				error: "Slug must be 3–64 chars: lowercase letters, numbers, hyphens.",
			});
		}
		if (logo && !/^https?:\/\//i.test(logo)) {
			return fail(400, { error: "Logo must be a https URL." });
		}

		// Slug uniqueness — let Postgres own the check, but surface a clean
		// error rather than a 500.
		try {
			await getAuth().api.updateOrganization({
				headers: request.headers,
				body: {
					organizationId: orgId,
					data: { name, slug, logo: logo ?? undefined },
				},
			});
		} catch (err) {
			const msg = (err as Error).message ?? "";
			if (msg.toLowerCase().includes("slug")) {
				return fail(400, { error: "That slug is already taken." });
			}
			return fail(400, { error: msg || "Couldn't update the team." });
		}
		return { ok: true };
	},

	invite: async ({ request }) => {
		const { orgId, myRole } = await loadActiveOrg(request.headers);
		if (!isManager(myRole)) return fail(403, { error: "Forbidden" });
		const fd = await request.formData();
		const email = String(fd.get("email") ?? "").trim().toLowerCase();
		const role = String(fd.get("role") ?? "member") as "member" | "admin";
		if (!email) return fail(400, { error: "Email required" });
		if (role !== "member" && role !== "admin") {
			return fail(400, { error: "Invalid role" });
		}

		// Public sign-up is closed in production, so a brand-new invitee who
		// isn't on the waitlist couldn't otherwise create an account to
		// accept the invitation. Pre-create a `status=active` user row so the
		// magic-link sign-in (and /accept-invitation) flow has something to
		// find. If a row already exists — active, pending, or banned — we
		// leave it untouched. Pending invitees get promoted to active so the
		// invitation supersedes the waitlist queue.
		const db = getDb();
		const [existing] = await db
			.select({ id: userTable.id, status: userTable.status })
			.from(userTable)
			.where(eq(userTable.email, email))
			.limit(1);
		if (!existing) {
			await db.insert(userTable).values({
				id: crypto.randomUUID(),
				email,
				name: email.split("@")[0]!,
				status: "active",
				// Owner-vouched: the invite IS the verification. Skipping this
				// makes Better Auth's accept-invitation reject the session with
				// "email not verified" when the invitee signs in for the first
				// time via magic link.
				emailVerified: true,
			});
		} else if (existing.status === "pending") {
			await db
				.update(userTable)
				.set({ status: "active", emailVerified: true })
				.where(eq(userTable.id, existing.id));
		}

		try {
			await getAuth().api.createInvitation({
				headers: request.headers,
				body: { email, role, organizationId: orgId },
			});
		} catch (err) {
			return fail(400, { error: (err as Error).message ?? "Couldn't send invite" });
		}
		return { ok: true };
	},

	cancelInvite: async ({ request }) => {
		const { myRole } = await loadActiveOrg(request.headers);
		if (!isManager(myRole)) return fail(403, { error: "Forbidden" });
		const fd = await request.formData();
		const id = String(fd.get("id") ?? "");
		if (!id) return fail(400, { error: "Missing invite id" });
		await getAuth().api.cancelInvitation({
			headers: request.headers,
			body: { invitationId: id },
		});
		return { ok: true };
	},

	updateRole: async ({ request }) => {
		const { orgId, myRole } = await loadActiveOrg(request.headers);
		if (!isManager(myRole)) return fail(403, { error: "Forbidden" });
		const fd = await request.formData();
		const memberId = String(fd.get("memberId") ?? "");
		const role = String(fd.get("role") ?? "") as "owner" | "admin" | "member";
		if (!memberId || !["owner", "admin", "member"].includes(role)) {
			return fail(400, { error: "Invalid request" });
		}
		await getAuth().api.updateMemberRole({
			headers: request.headers,
			body: { memberId, role, organizationId: orgId },
		});
		return { ok: true };
	},

	removeMember: async ({ request }) => {
		const { orgId, myRole } = await loadActiveOrg(request.headers);
		if (!isManager(myRole)) return fail(403, { error: "Forbidden" });
		const fd = await request.formData();
		const memberIdOrEmail = String(fd.get("memberIdOrEmail") ?? "");
		if (!memberIdOrEmail) return fail(400, { error: "Missing member" });
		await getAuth().api.removeMember({
			headers: request.headers,
			body: { memberIdOrEmail, organizationId: orgId },
		});
		return { ok: true };
	},

	leave: async ({ request }) => {
		const { orgId } = await loadActiveOrg(request.headers);
		await getAuth().api.leaveOrganization({
			headers: request.headers,
			body: { organizationId: orgId },
		});
		redirect(303, "/dashboard");
	},
};
