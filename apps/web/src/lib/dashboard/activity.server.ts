/**
 * Real viewer-activity loader. Reads the `share_view` rows the player records
 * (via POST /api/share/[id]/view) and the workspace's shares, joins them up to
 * the owning recast, and projects both into the shared `Activity` shape the
 * Home + Analytics surfaces render. Replaces the old deterministic mock.
 */

import { and, desc, eq, isNull, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { recast, share, shareComment, shareReaction, shareView } from "$lib/db/schema";
import { reactionGlyph } from "$lib/share/reactions";
import { deviceFromUA } from "$lib/share/ua";
import type { Activity, EngagementMoment, RecastEngagement, RecastPerf } from "./activity";

let regionNames: Intl.DisplayNames | null = null;
function viewerLabel(country: string | null): string {
	if (!country) return "Anonymous viewer";
	try {
		regionNames ??= new Intl.DisplayNames(["en"], { type: "region" });
		const name = regionNames.of(country.toUpperCase());
		return name ? `Viewer from ${name}` : "Anonymous viewer";
	} catch {
		return "Anonymous viewer";
	}
}

type ViewRow = {
	id: string;
	recastId: string;
	recastTitle: string;
	sessionId: string;
	country: string | null;
	device: string | null;
	userAgent: string | null;
	referrer: string | null;
	completed: boolean;
	watchPct: number;
	createdAt: Date | null;
};

function viewRowToActivity(r: ViewRow): Activity {
	return {
		id: r.id,
		recastId: r.recastId,
		recastTitle: r.recastTitle,
		viewer: viewerLabel(r.country),
		sessionId: r.sessionId,
		country: r.country,
		// Prefer the stored device; back-fill from the UA for rows written before
		// the `device` column existed so historical breakdowns still populate.
		device: r.device ?? deviceFromUA(r.userAgent),
		referrer: r.referrer,
		kind: r.completed ? "completed" : "viewed",
		timestamp: (r.createdAt ?? new Date(0)).getTime(),
		watchPct: r.watchPct,
	};
}

/** Columns every view query needs to build an `Activity` (kept in sync so the
 *  workspace + per-recast loaders project identically). */
const viewSelection = {
	id: shareView.id,
	recastId: recast.id,
	recastTitle: recast.title,
	sessionId: shareView.sessionId,
	country: shareView.country,
	device: shareView.device,
	userAgent: shareView.userAgent,
	referrer: shareView.referrer,
	completed: shareView.completed,
	watchPct: shareView.watchPct,
	createdAt: shareView.createdAt,
} as const;

/**
 * Load the workspace's viewer activity — view/completion events plus
 * share-created events — newest first. `limit` caps each source independently;
 * the merged list is what the pages slice by date range.
 */
export async function loadWorkspaceActivity(
	workspaceId: string,
	limit = 250,
): Promise<Activity[]> {
	const db = getDb();

	const [viewRows, shareRows] = await Promise.all([
		db
			.select(viewSelection)
			.from(shareView)
			.innerJoin(share, eq(shareView.shareId, share.slug))
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(eq(recast.workspaceId, workspaceId))
			.orderBy(desc(shareView.createdAt))
			.limit(limit),
		db
			.select({
				slug: share.slug,
				recastId: recast.id,
				recastTitle: recast.title,
				createdAt: share.createdAt,
			})
			.from(share)
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(eq(recast.workspaceId, workspaceId))
			.orderBy(desc(share.createdAt))
			.limit(limit),
	]);

	const views: Activity[] = viewRows.map(viewRowToActivity);

	const shares: Activity[] = shareRows.map((r) => ({
		id: `${r.slug}-shared`,
		recastId: r.recastId,
		recastTitle: r.recastTitle,
		viewer: "You",
		kind: "shared",
		timestamp: (r.createdAt ?? new Date(0)).getTime(),
		watchPct: 0,
	}));

	return [...views, ...shares].sort((a, b) => b.timestamp - a.timestamp);
}

/**
 * Same projection as `loadWorkspaceActivity` but scoped to a single recast —
 * powers the per-recast detail page's chart, retention curve, and feed.
 */
export async function loadRecastActivity(
	recastId: string,
	limit = 500,
): Promise<Activity[]> {
	const db = getDb();

	const [viewRows, shareRows] = await Promise.all([
		db
			.select(viewSelection)
			.from(shareView)
			.innerJoin(share, eq(shareView.shareId, share.slug))
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(eq(recast.id, recastId))
			.orderBy(desc(shareView.createdAt))
			.limit(limit),
		db
			.select({
				slug: share.slug,
				recastId: recast.id,
				recastTitle: recast.title,
				createdAt: share.createdAt,
			})
			.from(share)
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(eq(recast.id, recastId))
			.orderBy(desc(share.createdAt)),
	]);

	const views: Activity[] = viewRows.map(viewRowToActivity);
	const shares: Activity[] = shareRows.map((r) => ({
		id: `${r.slug}-shared`,
		recastId: r.recastId,
		recastTitle: r.recastTitle,
		viewer: "You",
		kind: "shared",
		timestamp: (r.createdAt ?? new Date(0)).getTime(),
		watchPct: 0,
	}));

	return [...views, ...shares].sort((a, b) => b.timestamp - a.timestamp);
}

/**
 * Comments + reactions for a recast (across all its shares). This data is
 * collected by the player but was never surfaced to owners — the detail page
 * is where it closes the feedback loop.
 */
export async function loadRecastEngagement(recastId: string): Promise<RecastEngagement> {
	const db = getDb();

	const [commentRows, reactionRows] = await Promise.all([
		db
			.select({
				authorName: shareComment.authorName,
				body: shareComment.body,
				atSeconds: shareComment.atSeconds,
				createdAt: shareComment.createdAt,
			})
			.from(shareComment)
			.innerJoin(share, eq(shareComment.shareSlug, share.slug))
			.where(and(eq(share.recastId, recastId), isNull(shareComment.deletedAt)))
			.orderBy(desc(shareComment.createdAt)),
		db
			.select({ emoji: shareReaction.emoji, atSeconds: shareReaction.atSeconds })
			.from(shareReaction)
			.innerJoin(share, eq(shareReaction.shareSlug, share.slug))
			.where(eq(share.recastId, recastId)),
	]);

	const counts = new Map<string, number>();
	for (const r of reactionRows) counts.set(r.emoji, (counts.get(r.emoji) ?? 0) + 1);

	// Every reaction + comment with its video timestamp — the heatmap input.
	// Stored reaction values are registry ids; map to a display glyph (legacy
	// rows that stored a bare emoji pass through unchanged).
	const moments: EngagementMoment[] = [
		...reactionRows.map((r) => ({ atSeconds: r.atSeconds, kind: "reaction" as const, emoji: reactionGlyph(r.emoji) })),
		...commentRows.map((c) => ({ atSeconds: c.atSeconds, kind: "comment" as const })),
	];

	return {
		commentCount: commentRows.length,
		reactionCount: reactionRows.length,
		reactions: [...counts.entries()]
			.map(([emoji, count]) => ({ emoji: reactionGlyph(emoji), count }))
			.sort((a, b) => b.count - a.count),
		recentComments: commentRows.slice(0, 20).map((c) => ({
			authorName: c.authorName,
			body: c.body,
			atSeconds: c.atSeconds,
			createdAt: (c.createdAt ?? new Date(0)).getTime(),
		})),
		moments,
	};
}

/**
 * Per-recast performance rollups for the workspace analytics comparison table:
 * play count, average watch %, completion %, and comment count, grouped in two
 * aggregate queries (no N+1). Returns a recastId → metrics map.
 */
export async function loadWorkspacePerformance(
	workspaceId: string,
): Promise<Map<string, RecastPerf>> {
	const db = getDb();

	const [watch, comments] = await Promise.all([
		db
			.select({
				recastId: recast.id,
				views: sql<number>`count(*)`,
				avgWatch: sql<number>`coalesce(avg(${shareView.watchPct}), 0)`,
				completion: sql<number>`coalesce(avg((${shareView.completed})::int) * 100, 0)`,
			})
			.from(shareView)
			.innerJoin(share, eq(shareView.shareId, share.slug))
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(eq(recast.workspaceId, workspaceId))
			.groupBy(recast.id),
		db
			.select({ recastId: recast.id, comments: sql<number>`count(*)` })
			.from(shareComment)
			.innerJoin(share, eq(shareComment.shareSlug, share.slug))
			.innerJoin(recast, eq(share.recastId, recast.id))
			.where(and(eq(recast.workspaceId, workspaceId), isNull(shareComment.deletedAt)))
			.groupBy(recast.id),
	]);

	const map = new Map<string, RecastPerf>();
	for (const w of watch) {
		map.set(w.recastId, {
			views: Number(w.views),
			avgWatch: Math.round(Number(w.avgWatch)),
			completion: Math.round(Number(w.completion)),
			comments: 0,
		});
	}
	for (const c of comments) {
		const e = map.get(c.recastId) ?? { views: 0, avgWatch: 0, completion: 0, comments: 0 };
		e.comments = Number(c.comments);
		map.set(c.recastId, e);
	}
	return map;
}
