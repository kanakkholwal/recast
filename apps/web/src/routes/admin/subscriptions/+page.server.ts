import { desc, eq } from "drizzle-orm";
import { requireAdmin } from "$lib/admin/guard";
import { getDb } from "$lib/db";
import { subscription, user } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const db = getDb();
	// Streamed — the page header renders immediately while the table fills in.
	const rows = db
		.select({
			sub: subscription,
			user: { id: user.id, email: user.email, name: user.name },
		})
		.from(subscription)
		.innerJoin(user, eq(subscription.userId, user.id))
		.orderBy(desc(subscription.updatedAt))
		.limit(200);
	return { rows };
};
