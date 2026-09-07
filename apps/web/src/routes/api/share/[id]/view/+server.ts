import { error, json } from "@sveltejs/kit";
import { and, desc, eq, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { recast, share, shareView } from "$lib/db/schema";
import { enforceRateLimit } from "$lib/server/rate-limit";
import { deviceFromUA, referrerHost } from "$lib/share/ua";
import type { RequestHandler } from "./$types";

/**
 * POST /api/share/[id]/view
 *
 * Records viewer engagement. Two events from the player:
 *   - "start": a new view began — inserts a `share_view` row, bumps the
 *     cached `share.viewsCount`, and stamps `recast.lastViewedAt`.
 *   - "ended": playback finished — refreshes `lastViewedAt` and marks the
 *     session's latest row complete (watch %).
 *
 * `lastViewedAt` is the important one: the Free-tier expiry sweep archives
 * recasts with no views in N days off this column, so without these writes
 * every Free recast would expire regardless of real engagement.
 *
 * Identity is the anonymous `sessionId` fingerprint — viewers never need an
 * account. We don't gate on visibility here: the client only calls this once
 * the page loader has already granted access, and the worst a forged call can
 * do is inflate a view count / keep a recast alive, which is low-stakes.
 *
 * Body: { sessionId, event: "start" | "ended", watchPct? }
 */
export const POST: RequestHandler = async ({ params, request, getClientAddress }) => {
	// Per share and IP cap on engagement writes, so a forged loop can't inflate views or dodge the Free-tier expiry sweep.
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "share-view", id: params.id, limit: 40, windowMs: 60_000 },
	);
	if (limited) return limited;

	let body: {
		sessionId?: unknown;
		event?: unknown;
		watchPct?: unknown;
		referrer?: unknown;
	} = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const sessionId = typeof body.sessionId === "string" ? body.sessionId.trim().slice(0, 128) : "";
	if (!sessionId) error(400, "Missing session");
	const event = body.event === "ended" ? "ended" : "start";
	const watchPct =
		typeof body.watchPct === "number" && Number.isFinite(body.watchPct)
			? Math.min(100, Math.max(0, Math.round(body.watchPct)))
			: 0;

	const db = getDb();
	const [s] = await db
		.select({
			slug: share.slug,
			recastId: share.recastId,
			expiresAt: share.expiresAt,
		})
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);
	if (!s) error(404, "Share not found");
	if (s.expiresAt && s.expiresAt.getTime() < Date.now()) {
		return json({ ok: false, reason: "expired" }, { status: 410 });
	}

	// Best-effort geo and UA from edge headers, truncated so a hostile UA can't bloat the row; device is derived for a cheap GROUP BY.
	const country =
		request.headers.get("x-vercel-ip-country") ?? request.headers.get("cf-ipcountry") ?? null;
	const userAgent = request.headers.get("user-agent")?.slice(0, 512) ?? null;
	const device = deviceFromUA(userAgent);
	const referrer = referrerHost(
		typeof body.referrer === "string" ? body.referrer : null,
		request.url,
	);
	const now = new Date();

	await db.transaction(async (tx) => {
		if (event === "start") {
			await tx.insert(shareView).values({
				id: crypto.randomUUID(),
				shareId: s.slug,
				sessionId,
				country,
				userAgent,
				device,
				referrer,
				watchPct,
				completed: false,
			});
			await tx
				.update(share)
				.set({ viewsCount: sql`${share.viewsCount} + 1` })
				.where(eq(share.slug, s.slug));
		} else {
			// 'ended' refines this session's latest row; a view that ends with no recorded start just refreshes lastViewedAt.
			const [latest] = await tx
				.select({ id: shareView.id })
				.from(shareView)
				.where(and(eq(shareView.shareId, s.slug), eq(shareView.sessionId, sessionId)))
				.orderBy(desc(shareView.createdAt))
				.limit(1);
			if (latest) {
				await tx
					.update(shareView)
					.set({
						watchPct: sql`GREATEST(${shareView.watchPct}, ${watchPct})`,
						completed: true,
					})
					.where(eq(shareView.id, latest.id));
			}
		}

		await tx.update(recast).set({ lastViewedAt: now }).where(eq(recast.id, s.recastId));
	});

	return json({ ok: true });
};
