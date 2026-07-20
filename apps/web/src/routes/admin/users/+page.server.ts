import { fail } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { logAudit } from "$lib/admin/audit";
import { requireAdmin } from "$lib/admin/guard";
import { createSetPasswordLink } from "$lib/auth/invite";
import { firstNameOf, inviteDisplayName } from "$lib/auth/invite.logic";
import { ensureDefaultTeamForUser, getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { user } from "$lib/db/schema";
import { sendTemplatedEmail } from "$lib/email";
import { isValidEmail, normalizeEmail } from "$lib/validation/email";
import type { Actions, PageServerLoad } from "./$types";

/**
 * Server-side load that proxies straight to the admin plugin's listUsers
 * endpoint. We pass the caller's headers along so Better Auth can verify
 * the session (the plugin's middleware enforces admin-only access — our
 * `requireAdmin` is belt-and-braces in case the plugin config changes).
 *
 * The list query is returned as an un-awaited promise so SvelteKit streams it
 * — the filter bar + pagination shell render immediately while the table
 * fills in.
 */
export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);

	const url = event.url;
	const limit = Math.min(Math.max(Number(url.searchParams.get("limit") ?? 25), 5), 100);
	const offset = Math.max(Number(url.searchParams.get("offset") ?? 0), 0);
	const searchValue = url.searchParams.get("q")?.trim() ?? "";
	const searchField = (url.searchParams.get("field") as "email" | "name") ?? "email";
	const sortBy = url.searchParams.get("sort") ?? "createdAt";
	const sortDirection =
		url.searchParams.get("dir") === "asc" ? "asc" : ("desc" as const);
	const roleFilter = url.searchParams.get("role")?.trim() || null;
	const statusFilter = url.searchParams.get("status")?.trim() || null;

	const query: Record<string, unknown> = {
		limit,
		offset,
		sortBy,
		sortDirection,
	};
	if (searchValue) {
		query.searchValue = searchValue;
		query.searchField = searchField;
		query.searchOperator = "contains";
	}
	// listUsers supports a single filter — combine with the search query when
	// both are present. Role takes priority over status for the explicit filter.
	if (roleFilter) {
		query.filterField = "role";
		query.filterValue = roleFilter;
		query.filterOperator = "eq";
	} else if (statusFilter) {
		query.filterField = "status";
		query.filterValue = statusFilter;
		query.filterOperator = "eq";
	}

	// The plugin's listUsers types are dynamic; cast to the public shape we
	// actually consume below. Schema-level validation already happened above.
	type ListUsersResult = {
		users: Array<{
			id: string;
			email: string;
			name: string;
			role?: string | null;
			status?: string | null;
			banned?: boolean | null;
			banReason?: string | null;
			banExpires?: Date | string | null;
			createdAt: Date | string;
			emailVerified?: boolean;
		}>;
		total: number;
	};

	const list: Promise<ListUsersResult> = getAuth().api.listUsers({
		headers: event.request.headers,
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		query: query as any,
	}) as Promise<ListUsersResult>;

	return {
		list,
		limit,
		offset,
		filters: {
			q: searchValue,
			field: searchField,
			role: roleFilter,
			status: statusFilter,
			sort: sortBy,
			dir: sortDirection,
		},
	};
};

export const actions: Actions = {
	/**
	 * Invite a user directly, skipping the waitlist entirely. Creates them
	 * `active` (a `pending` row would silently suppress every outbound auth
	 * email via the isOnWaitlist gate in auth/server.ts) and mails them a
	 * set-password link.
	 *
	 * The user row is written straight through Drizzle rather than
	 * `auth.api.createUser`, because that endpoint requires a password and we
	 * want the invitee to choose their own. Better Auth's reset endpoint
	 * creates the `credential` account when they do.
	 */
	invite: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const rawEmail = String(fd.get("email") ?? "");
		const rawName = String(fd.get("name") ?? "");

		if (!isValidEmail(rawEmail)) return fail(400, { error: "Enter a valid email address." });
		const email = normalizeEmail(rawEmail);
		const name = inviteDisplayName(rawName, email);

		const db = getDb();
		const [existing] = await db
			.select({ id: user.id, status: user.status })
			.from(user)
			.where(eq(user.email, email))
			.limit(1);

		// An existing pending row means they signed up for the waitlist. Inviting
		// them is the same as approving them, so activate rather than reject.
		if (existing && existing.status !== "pending") {
			return fail(400, { error: "That email already has an account." });
		}

		let userId: string;
		if (existing) {
			userId = existing.id;
			await db
				.update(user)
				.set({ status: "active", updatedAt: new Date() })
				.where(eq(user.id, userId));
		} else {
			userId = crypto.randomUUID();
			await db.insert(user).values({ id: userId, email, name, status: "active" });
		}

		// New rows get a team from the user.create hook, but a promoted pending
		// row was skipped while it was on the waitlist.
		await ensureDefaultTeamForUser({ id: userId, name, email });

		await logAudit({
			actorId: admin.user.id,
			action: "user.invite",
			targetUserId: userId,
			metadata: { email, promotedFromWaitlist: Boolean(existing) },
		});

		// The account exists either way. Surface a send failure so the admin can
		// resend instead of assuming the invite landed.
		try {
			const url = await createSetPasswordLink(userId);
			await sendTemplatedEmail({
				to: email,
				template: "admin-invite",
				data: {
					url,
					firstName: firstNameOf(name),
					inviterName: admin.user.name || admin.user.email,
				},
			});
		} catch (err) {
			console.error("[admin] invite email failed", { email, err });
			return fail(502, {
				error: `${email} was created, but the invite email failed to send.`,
			});
		}

		return { ok: true, invited: email };
	},
};
