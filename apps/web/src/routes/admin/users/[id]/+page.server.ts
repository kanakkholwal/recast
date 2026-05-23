import { error, fail, redirect } from "@sveltejs/kit";
import { desc, eq } from "drizzle-orm";
import { logAudit } from "$lib/admin/audit";
import { requireAdmin } from "$lib/admin/guard";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { auditLog, session as sessionTable, subscription, user } from "$lib/db/schema";
import type { Actions, PageServerLoad } from "./$types";

/**
 * Per-user admin page. `target` must resolve before render — it gates the 404
 * and seeds the page header — so we await it. Sessions, sub and audit log live
 * in collapsibles below the fold and are streamed.
 */
export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const id = event.params.id;
	const db = getDb();

	const [target] = await db.select().from(user).where(eq(user.id, id)).limit(1);
	if (!target) error(404, "User not found");

	const sessions = db
		.select()
		.from(sessionTable)
		.where(eq(sessionTable.userId, id))
		.orderBy(desc(sessionTable.createdAt))
		.limit(20);

	const sub = (async () => {
		const [row] = await db
			.select()
			.from(subscription)
			.where(eq(subscription.userId, id))
			.limit(1);
		return row ?? null;
	})();

	const audit = db
		.select()
		.from(auditLog)
		.where(eq(auditLog.targetUserId, id))
		.orderBy(desc(auditLog.createdAt))
		.limit(20);

	return { target, sessions, sub, audit };
};

/**
 * All admin actions on a single user. Every action:
 *   1. Re-verifies the caller is admin (the plugin will too, but we redirect
 *      cleanly rather than letting the API throw).
 *   2. Calls the matching `auth.api.*` endpoint (or direct Drizzle for
 *      app-owned columns like `status`).
 *   3. Writes an audit entry on success.
 *
 * Keep this list lean — the plugin's endpoints are the source of truth for
 * what "an action" is.
 */
export const actions: Actions = {
	updateProfile: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const name = String(fd.get("name") ?? "").trim();
		if (!name) return fail(400, { error: "Name required" });

		// `auth.api.updateUser` is the *self-update* endpoint — it operates on
		// the calling session's user. There's no admin-side "update arbitrary
		// user's profile" endpoint in the plugin, so we update the row
		// directly. Stays safe because requireAdmin() ran above and the
		// column set is small (just `name`, by design).
		await getDb().update(user).set({ name, updatedAt: new Date() }).where(eq(user.id, id));
		await logAudit({
			actorId: admin.user.id,
			action: "user.update",
			targetUserId: id,
			metadata: { name },
		});
		return { ok: true };
	},

	setRole: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const role = String(fd.get("role") ?? "") as "user" | "admin";
		if (role !== "user" && role !== "admin") {
			return fail(400, { error: "Invalid role" });
		}

		await getAuth().api.setRole({
			headers: event.request.headers,
			body: { userId: id, role },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "user.set_role",
			targetUserId: id,
			metadata: { role },
		});
		return { ok: true };
	},

	setStatus: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const status = String(fd.get("status") ?? "");
		if (!["active", "pending"].includes(status)) {
			return fail(400, { error: "Invalid status" });
		}
		// `status` is app-owned (not managed by the admin plugin), so we
		// update Drizzle directly.
		await getDb().update(user).set({ status, updatedAt: new Date() }).where(eq(user.id, id));
		await logAudit({
			actorId: admin.user.id,
			action: status === "active" ? "waitlist.approve" : "user.update",
			targetUserId: id,
			metadata: { status },
		});
		return { ok: true };
	},

	ban: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const reason = String(fd.get("reason") ?? "").trim() || "Banned by admin";
		const days = Math.max(0, Number(fd.get("expiresInDays") ?? 0));
		const banExpiresIn = days > 0 ? days * 86400 : undefined;

		await getAuth().api.banUser({
			headers: event.request.headers,
			body: { userId: id, banReason: reason, banExpiresIn },
		});
		// Banning doesn't auto-revoke existing sessions per Better Auth docs;
		// kill them too so the user is logged out immediately.
		await getAuth().api.revokeUserSessions({
			headers: event.request.headers,
			body: { userId: id },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "user.ban",
			targetUserId: id,
			metadata: { reason, days },
		});
		return { ok: true };
	},

	unban: async (event) => {
		const admin = await requireAdmin(event);
		const id = event.params.id;
		await getAuth().api.unbanUser({
			headers: event.request.headers,
			body: { userId: id },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "user.unban",
			targetUserId: id,
		});
		return { ok: true };
	},

	setPassword: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const id = event.params.id;
		const newPassword = String(fd.get("password") ?? "");
		if (newPassword.length < 8) {
			return fail(400, { error: "Password must be ≥8 chars" });
		}
		await getAuth().api.setUserPassword({
			headers: event.request.headers,
			body: { userId: id, newPassword },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "user.set_password",
			targetUserId: id,
		});
		return { ok: true };
	},

	revokeSession: async (event) => {
		const admin = await requireAdmin(event);
		const fd = await event.request.formData();
		const sessionToken = String(fd.get("sessionToken") ?? "");
		if (!sessionToken) return fail(400, { error: "Missing sessionToken" });
		await getAuth().api.revokeUserSession({
			headers: event.request.headers,
			body: { sessionToken },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "session.revoke",
			targetUserId: event.params.id,
		});
		return { ok: true };
	},

	revokeAllSessions: async (event) => {
		const admin = await requireAdmin(event);
		const id = event.params.id;
		await getAuth().api.revokeUserSessions({
			headers: event.request.headers,
			body: { userId: id },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "session.revoke",
			targetUserId: id,
			metadata: { all: true },
		});
		return { ok: true };
	},

	remove: async (event) => {
		const admin = await requireAdmin(event);
		const id = event.params.id;
		if (id === admin.user.id) return fail(400, { error: "You can't delete yourself." });
		await getAuth().api.removeUser({
			headers: event.request.headers,
			body: { userId: id },
		});
		await logAudit({
			actorId: admin.user.id,
			action: "user.delete",
			targetUserId: id,
		});
		redirect(303, "/admin/users");
	},
};
