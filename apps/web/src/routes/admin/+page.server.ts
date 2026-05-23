import { count, desc, eq, gte, inArray, type SQL } from "drizzle-orm";
import { getDb } from "$lib/db";
import { auditLog, subscription, user, type AuditLog } from "$lib/db/schema";
import type { PageServerLoad } from "./$types";

/**
 * Admin overview. Each section is returned as a top-level promise so SvelteKit
 * streams it to the client — the shell + skeletons render immediately, the
 * cards/lists fill in as their queries resolve.
 *
 * We run multiple small `count()` queries rather than one aggregate with
 * `FILTER (WHERE ...)` — pgbouncer's transaction-pooling mode + `prepare: false`
 * doesn't always play nicely with FILTER, and a per-metric failure surfaces a
 * specific Postgres error instead of one opaque "Failed query".
 */
export const load: PageServerLoad = async () => {
	const db = getDb();
	const now = new Date();
	const sevenDaysAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
	const thirtyDaysAgo = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);

	async function safeCount(label: string, run: () => Promise<number>): Promise<number> {
		try {
			return await run();
		} catch (err) {
			console.error(`[admin overview] ${label} count failed:`, (err as Error).message);
			return 0;
		}
	}

	const countWhere = async (cond: SQL) =>
		(await db.select({ c: count() }).from(user).where(cond))[0]?.c ?? 0;

	const metrics = (async () => {
		const [
			total,
			active,
			pending,
			admins,
			banned,
			signups7d,
			signups30d,
			subsTotal,
			subsActive,
		] = await Promise.all([
			safeCount(
				"total",
				async () => (await db.select({ c: count() }).from(user))[0]?.c ?? 0,
			),
			safeCount("active", () => countWhere(eq(user.status, "active"))),
			safeCount("pending", () => countWhere(eq(user.status, "pending"))),
			safeCount("admins", () => countWhere(eq(user.role, "admin"))),
			safeCount("banned", () => countWhere(eq(user.banned, true))),
			safeCount("signups7d", () => countWhere(gte(user.createdAt, sevenDaysAgo))),
			safeCount("signups30d", () => countWhere(gte(user.createdAt, thirtyDaysAgo))),
			safeCount(
				"subsTotal",
				async () => (await db.select({ c: count() }).from(subscription))[0]?.c ?? 0,
			),
			safeCount(
				"subsActive",
				async () =>
					(
						await db
							.select({ c: count() })
							.from(subscription)
							.where(inArray(subscription.status, ["active", "trialing"] as const))
					)[0]?.c ?? 0,
			),
		]);
		return {
			counts: { total, active, pending, admins, banned, signups7d, signups30d },
			subs: { total: subsTotal, active: subsActive },
		};
	})();

	const recentAudit: Promise<AuditLog[]> = (async () => {
		try {
			return await db
				.select()
				.from(auditLog)
				.orderBy(desc(auditLog.createdAt))
				.limit(8);
		} catch (err) {
			console.error("[admin overview] recent audit failed:", (err as Error).message);
			return [];
		}
	})();

	const recentUsers = (async () => {
		try {
			return await db
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
		} catch (err) {
			console.error("[admin overview] recent users failed:", (err as Error).message);
			return [] as Array<{
				id: string;
				email: string;
				name: string;
				role: string;
				status: string;
				createdAt: Date;
			}>;
		}
	})();

	return {
		metrics,
		recentAudit,
		recentUsers,
	};
};
