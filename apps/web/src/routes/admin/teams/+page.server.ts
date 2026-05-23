import { count, desc, eq, sql } from "drizzle-orm";
import { requireAdmin } from "$lib/admin/guard";
import { getDb } from "$lib/db";
import { member as memberTable, organization as organizationTable } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);
	const db = getDb();

	// Streamed — the page header renders immediately while the table fills in.
	const teams = db
		.select({
			id: organizationTable.id,
			name: organizationTable.name,
			slug: organizationTable.slug,
			plan: organizationTable.plan,
			createdAt: organizationTable.createdAt,
			memberCount:
				sql<number>`(select count(*)::int from ${memberTable} where ${memberTable.organizationId} = ${organizationTable.id})`.mapWith(
					Number,
				),
		})
		.from(organizationTable)
		.orderBy(desc(organizationTable.createdAt))
		.limit(200);

	return { teams };
};
