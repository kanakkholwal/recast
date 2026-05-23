import { error, fail } from "@sveltejs/kit";
import { desc, eq } from "drizzle-orm";
import { logAudit } from "$lib/admin/audit";
import { requireAdmin } from "$lib/admin/guard";
import { getDb } from "$lib/db";
import {
	TEAM_PLAN_MEMBER_CAPS,
	member as memberTable,
	organization as organizationTable,
	user as userTable,
} from "$lib/db/schema";
import type { Actions, PageServerLoad } from "./$types";

/** Confirm the org id from the URL is still a real row before any mutation
 *  + audit-log writes. A stale tab submitting against a deleted team must not
 *  silently succeed and leave a phantom entry behind. */
async function ensureTeamExists(id: string): Promise<boolean> {
	const [row] = await getDb()
		.select({ id: organizationTable.id })
		.from(organizationTable)
		.where(eq(organizationTable.id, id))
		.limit(1);
	return Boolean(row);
}

export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const db = getDb();
	const id = event.params.id;

	const [team] = await db
		.select()
		.from(organizationTable)
		.where(eq(organizationTable.id, id))
		.limit(1);
	if (!team) error(404, "Team not found");

	// Members streamed — team metadata (name, plan, slug) renders immediately,
	// the member list fills in.
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
		.where(eq(memberTable.organizationId, id))
		.orderBy(desc(memberTable.createdAt));

	return {
		team,
		members,
		caps: { members: TEAM_PLAN_MEMBER_CAPS },
	};
};

export const actions: Actions = {
	updatePlan: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const plan = String(fd.get("plan") ?? "") as "free" | "pro" | "enterprise";
		if (!["free", "pro", "enterprise"].includes(plan)) {
			return fail(400, { error: "Invalid plan" });
		}
		if (!(await ensureTeamExists(id))) error(404, "Team not found");
		await getDb()
			.update(organizationTable)
			.set({ plan })
			.where(eq(organizationTable.id, id));
		await logAudit({
			actorId: admin.user.id,
			action: "team.update_plan",
			targetUserId: null,
			metadata: { teamId: id, plan },
		});
		return { ok: true };
	},

	rename: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const name = String(fd.get("name") ?? "").trim();
		if (!name) return fail(400, { error: "Name required" });
		if (!(await ensureTeamExists(id))) error(404, "Team not found");
		await getDb()
			.update(organizationTable)
			.set({ name })
			.where(eq(organizationTable.id, id));
		await logAudit({
			actorId: admin.user.id,
			action: "team.rename",
			targetUserId: null,
			metadata: { teamId: id, name },
		});
		return { ok: true };
	},
};
