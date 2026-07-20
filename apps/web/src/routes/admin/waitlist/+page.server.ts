import { fail } from "@sveltejs/kit";
import { desc, eq, inArray } from "drizzle-orm";
import { logAudit } from "$lib/admin/audit";
import { requireAdmin } from "$lib/admin/guard";
import { createSetPasswordLink } from "$lib/auth/invite";
import { firstNameOf } from "$lib/auth/invite.logic";
import { ensureDefaultTeamForUser } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { user } from "$lib/db/schema";
import { sendTemplatedEmail } from "$lib/email";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const db = getDb();
	// Streamed — the page header and approve form render immediately while
	// the list fills in.
	const pending = db
		.select({
			id: user.id,
			email: user.email,
			name: user.name,
			createdAt: user.createdAt,
		})
		.from(user)
		.where(eq(user.status, "pending"))
		.orderBy(desc(user.createdAt))
		.limit(200);
	return { pending };
};

export const actions: Actions = {
	approve: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const ids = fd.getAll("id").map(String).filter(Boolean);
		if (!ids.length) return fail(400, { error: "No users selected" });

		const db = getDb();
		// Read before the flip so we know who was actually pending. Re-approving
		// an already-active user shouldn't re-send them a welcome email.
		const targets = await db
			.select({ id: user.id, email: user.email, name: user.name, status: user.status })
			.from(user)
			.where(inArray(user.id, ids));
		const newlyApproved = targets.filter((t) => t.status === "pending");

		await db
			.update(user)
			.set({ status: "active", updatedAt: new Date() })
			.where(inArray(user.id, ids));

		for (const id of ids) {
			await logAudit({
				actorId: admin.user.id,
				action: "waitlist.approve",
				targetUserId: id,
			});
		}

		// Activation has to back-fill the default team: the user.create hook
		// skipped it while they were pending.
		let emailed = 0;
		for (const target of newlyApproved) {
			await ensureDefaultTeamForUser({
				id: target.id,
				name: target.name ?? "",
				email: target.email,
			});
			// One bad address shouldn't strand the rest of a bulk approve. The
			// status flip already committed, so a failure here costs the user
			// their email, not their account.
			try {
				const url = await createSetPasswordLink(target.id);
				await sendTemplatedEmail({
					to: target.email,
					template: "waitlist-approved",
					data: { url, firstName: firstNameOf(target.name) },
				});
				emailed++;
			} catch (err) {
				console.error("[waitlist] approve email failed", { id: target.id, err });
			}
		}

		return { ok: true, approved: ids.length, emailed };
	},
};
