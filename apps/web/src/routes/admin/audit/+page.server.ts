import { desc, eq } from "drizzle-orm";
import { requireAdmin } from "$lib/admin/guard";
import { getDb } from "$lib/db";
import { auditLog, user } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const limit = Math.min(Math.max(Number(event.url.searchParams.get("limit") ?? 100), 10), 500);
	const db = getDb();

	// Streamed — the page header renders immediately while the table fills in.
	const rows = db
		.select({
			id: auditLog.id,
			action: auditLog.action,
			metadata: auditLog.metadata,
			createdAt: auditLog.createdAt,
			actorId: auditLog.actorId,
			actorEmail: user.email,
			targetUserId: auditLog.targetUserId,
		})
		.from(auditLog)
		.leftJoin(user, eq(auditLog.actorId, user.id))
		.orderBy(desc(auditLog.createdAt))
		.limit(limit);

	return { rows };
};
