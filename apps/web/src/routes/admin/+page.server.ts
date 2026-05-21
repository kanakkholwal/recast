import { count, desc, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { auditLog, subscription, user } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async () => {
	const db = getDb();
	const now = new Date();
	const sevenDaysAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
	const thirtyDaysAgo = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);

	// Single aggregate query — Postgres handles all the filters cheaply.
	const [counts] = await db
		.select({
			total: count(user.id),
			active: sql<number>`count(*) filter (where ${user.status} = 'active')`.mapWith(Number),
			pending: sql<number>`count(*) filter (where ${user.status} = 'pending')`.mapWith(Number),
			admins: sql<number>`count(*) filter (where ${user.role} = 'admin')`.mapWith(Number),
			banned: sql<number>`count(*) filter (where ${user.banned} = true)`.mapWith(Number),
			signups7d: sql<number>`count(*) filter (where ${user.createdAt} >= ${sevenDaysAgo})`.mapWith(Number),
			signups30d: sql<number>`count(*) filter (where ${user.createdAt} >= ${thirtyDaysAgo})`.mapWith(Number),
		})
		.from(user);

	const [subs] = await db
		.select({
			total: count(subscription.id),
			active: sql<number>`count(*) filter (where ${subscription.status} in ('active', 'trialing'))`.mapWith(Number),
		})
		.from(subscription);

	const recentAudit = await db
		.select()
		.from(auditLog)
		.orderBy(desc(auditLog.createdAt))
		.limit(8);

	const recentUsers = await db
		.select({
			id: user.id,
			email: user.email,
			name: user.name,
			role: user.role,
			status: user.status,
			createdAt: user.createdAt,
		})
		.from(user)
		.orderBy(desc(user.createdAt))
		.limit(6);

	return {
		counts: counts ?? {
			total: 0,
			active: 0,
			pending: 0,
			admins: 0,
			banned: 0,
			signups7d: 0,
			signups30d: 0,
		},
		subs: subs ?? { total: 0, active: 0 },
		recentAudit,
		recentUsers,
	};
};
