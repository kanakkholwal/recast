import { fail } from "@sveltejs/kit";
import { desc, eq, inArray } from "drizzle-orm";
import { logAudit } from "$lib/admin/audit";
import { requireAdmin } from "$lib/admin/guard";
import { getDb } from "$lib/db";
import { user } from "$lib/db/schema";
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
		return { ok: true, approved: ids.length };
	},
};
