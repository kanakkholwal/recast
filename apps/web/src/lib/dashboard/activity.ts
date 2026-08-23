/**
 * Viewer-activity shapes + aggregation helpers for the Home and Analytics
 * surfaces. The events themselves are loaded server-side from the real
 * `share_view` table (see `activity.server.ts`) — this module only defines the
 * shape and the pure roll-ups the pages compute from a `range`-filtered slice.
 */

export type ActivityKind = "viewed" | "completed" | "shared" | "downloaded";

export type Activity = {
	id: string;
	recastId: string;
	recastTitle: string;
	/** Human label for the row ("Anonymous viewer", "Viewer from India", "You"). */
	viewer: string;
	/** Anonymous session fingerprint for view events — the key we count unique
	 *  viewers by. Absent on non-view rows (e.g. "shared"). */
	sessionId?: string;
	/** ISO country code from the edge header, when known (view events only). */
	country?: string | null;
	/** Coarse device class ("mobile" | "tablet" | "desktop"), view events only. */
	device?: string | null;
	/** Referring host the viewer arrived from, when known (view events only). */
	referrer?: string | null;
	kind: ActivityKind;
	timestamp: number;
	watchPct: number;
};

/** Only the "viewed"/"completed" rows represent an actual play. */
function viewEvents(activity: Activity[]): Activity[] {
	return activity.filter((a) => a.kind === "viewed" || a.kind === "completed");
}

/** A single engagement event anchored to a point in the video — the raw
 *  material for the "what moments did they react to" heatmap. */
export type EngagementMoment = {
	atSeconds: number;
	kind: "reaction" | "comment";
	/** Present on reactions only. */
	emoji?: string;
};

/** Comments + reactions rollup for one recast (see `loadRecastEngagement`). */
export type RecastEngagement = {
	commentCount: number;
	reactionCount: number;
	/** Emoji → count, most-used first. */
	reactions: { emoji: string; count: number }[];
	recentComments: { authorName: string; body: string; atSeconds: number; createdAt: number }[];
	/** Every reaction/comment with its video timestamp, for the timeline heatmap. */
	moments: EngagementMoment[];
};

/** Free-plan analytics payload. No per-viewer dimensions by construction. */
export type RecastBasicStats = {
	views: number;
	viewers: number;
	completionPct: number;
	byDay: { date: number; label: string; views: number }[];
};

/** Per-recast performance metrics for the workspace comparison table. */
export type RecastPerf = {
	views: number;
	avgWatch: number;
	completion: number;
	comments: number;
};

const DAY = 86_400_000;

/** Aggregate view events into daily buckets ending today. */
export function viewsByDay(
	activity: Activity[],
	days: number,
): { date: number; label: string; views: number }[] {
	const todayStart = new Date();
	todayStart.setHours(0, 0, 0, 0);
	const buckets: { date: number; label: string; views: number }[] = [];
	for (let i = days - 1; i >= 0; i--) {
		const d = new Date(todayStart);
		d.setDate(d.getDate() - i);
		buckets.push({
			date: d.getTime(),
			label:
				days <= 7
					? d.toLocaleDateString("en-US", { weekday: "short" })
					: d.toLocaleDateString("en-US", { month: "short", day: "numeric" }),
			views: 0,
		});
	}
	const start = buckets[0]!.date;
	for (const ev of activity) {
		if (ev.kind !== "viewed" && ev.kind !== "completed") continue;
		if (ev.timestamp < start) continue;
		const idx = Math.floor((ev.timestamp - start) / DAY);
		if (idx >= 0 && idx < buckets.length) buckets[idx]!.views++;
	}
	return buckets;
}

export function avgWatchPct(activity: Activity[]): number {
	const views = activity.filter((a) => a.kind === "viewed" || a.kind === "completed");
	if (views.length === 0) return 0;
	const sum = views.reduce((s, v) => s + v.watchPct, 0);
	return Math.round(sum / views.length);
}

/** Distinct viewers among view events, keyed by the anonymous session
 *  fingerprint (falls back to the display label when no session is attached). */
export function uniqueViewers(activity: Activity[]): number {
	const set = new Set<string>();
	for (const a of viewEvents(activity)) {
		set.add(a.sessionId ?? a.viewer);
	}
	return set.size;
}

/** % of plays that ran to the end (`completed`). The signal founders want:
 *  "do people actually finish?" — distinct from average watch %. */
export function completionRate(activity: Activity[]): number {
	const views = viewEvents(activity);
	if (views.length === 0) return 0;
	const finished = views.filter((a) => a.kind === "completed").length;
	return Math.round((finished / views.length) * 100);
}

/** Total play count (viewed + completed events). */
export function viewCount(activity: Activity[]): number {
	return viewEvents(activity).length;
}

/**
 * Watch-retention survival curve: for each decile threshold (10%…100%), the
 * share of plays that reached at least that far. Surfaces WHERE viewers drop
 * off rather than collapsing everything to one average. `reached` is 0–100.
 */
export function watchRetention(activity: Activity[]): { pct: number; reached: number }[] {
	const views = viewEvents(activity);
	const total = views.length;
	const steps = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
	return steps.map((pct) => {
		if (total === 0) return { pct, reached: 0 };
		const n = views.filter((v) => v.watchPct >= pct).length;
		return { pct, reached: Math.round((n / total) * 100) };
	});
}

/** A labelled slice of a breakdown (geography / device): a bucket, its count,
 *  and its share of the whole (0–100). */
export type BreakdownRow = { key: string; label: string; count: number; pct: number };

let regionNames: Intl.DisplayNames | null = null;
function countryName(code: string): string {
	try {
		regionNames ??= new Intl.DisplayNames(["en"], { type: "region" });
		return regionNames.of(code.toUpperCase()) ?? code.toUpperCase();
	} catch {
		return code.toUpperCase();
	}
}

/**
 * Audience by country, busiest first. Each unique session is counted once (by
 * its first/any view), so a viewer who replays five times is one person from
 * one place — the Instagram "reach by location" model, not raw play counts.
 * Rows without a known country collapse into a single "Unknown" bucket.
 */
export function geographyBreakdown(activity: Activity[], limit = 6): BreakdownRow[] {
	const seen = new Set<string>();
	const counts = new Map<string, number>();
	for (const a of viewEvents(activity)) {
		const session = a.sessionId ?? a.viewer;
		if (seen.has(session)) continue;
		seen.add(session);
		const code = (a.country ?? "").trim().toUpperCase() || "??";
		counts.set(code, (counts.get(code) ?? 0) + 1);
	}
	return finishBreakdown(counts, (code) => (code === "??" ? "Unknown" : countryName(code)), limit);
}

/**
 * Audience by device class (mobile / tablet / desktop), busiest first. Counted
 * per unique viewer like geography.
 */
export function deviceBreakdown(activity: Activity[]): BreakdownRow[] {
	const seen = new Set<string>();
	const counts = new Map<string, number>();
	for (const a of viewEvents(activity)) {
		const session = a.sessionId ?? a.viewer;
		if (seen.has(session)) continue;
		seen.add(session);
		const d = (a.device ?? "desktop").toLowerCase();
		const key = d === "mobile" || d === "tablet" ? d : "desktop";
		counts.set(key, (counts.get(key) ?? 0) + 1);
	}
	const label: Record<string, string> = { mobile: "Mobile", tablet: "Tablet", desktop: "Desktop" };
	return finishBreakdown(counts, (k) => label[k] ?? k, 3);
}

/**
 * Traffic sources — the referring hosts viewers arrived from, busiest first.
 * Null/empty referrers (direct opens, privacy-stripped) collapse into "Direct".
 * Counted per unique viewer like the other breakdowns.
 */
export function trafficBreakdown(activity: Activity[], limit = 6): BreakdownRow[] {
	const seen = new Set<string>();
	const counts = new Map<string, number>();
	for (const a of viewEvents(activity)) {
		const session = a.sessionId ?? a.viewer;
		if (seen.has(session)) continue;
		seen.add(session);
		const host = (a.referrer ?? "").trim().toLowerCase() || "__direct";
		counts.set(host, (counts.get(host) ?? 0) + 1);
	}
	return finishBreakdown(counts, (k) => (k === "__direct" ? "Direct / unknown" : k), limit);
}

function finishBreakdown(
	counts: Map<string, number>,
	label: (key: string) => string,
	limit: number,
): BreakdownRow[] {
	const total = [...counts.values()].reduce((s, n) => s + n, 0);
	if (total === 0) return [];
	const rows = [...counts.entries()]
		.map(([key, count]) => ({
			key,
			label: label(key),
			count,
			pct: Math.round((count / total) * 100),
		}))
		.sort((a, b) => b.count - a.count);
	if (rows.length <= limit) return rows;
	// Fold the long tail into one "Other" row so the bar list stays scannable.
	const head = rows.slice(0, limit - 1);
	const tail = rows.slice(limit - 1);
	const tailCount = tail.reduce((s, r) => s + r.count, 0);
	head.push({
		key: "__other",
		label: `Other (${tail.length})`,
		count: tailCount,
		pct: Math.round((tailCount / total) * 100),
	});
	return head;
}

/**
 * Instagram-style engagement rate: interactions (reactions + comments) per 100
 * views. Returns a rounded percentage; >100 is possible (a hot clip can draw
 * several reactions per view) and that's intentional signal, not a bug.
 */
export function engagementRate(views: number, reactions: number, comments: number): number {
	if (views <= 0) return 0;
	return Math.round(((reactions + comments) / views) * 100);
}

/**
 * Bucket engagement moments across the video's runtime into `buckets` equal
 * time slices, splitting reactions vs. comments per slice. Surfaces WHERE in
 * the video viewers react — the clearest "what they actually liked" signal.
 * `peakLabel` marks the hottest slice (e.g. "0:52"). Returns empty when there's
 * nothing to plot.
 */
export function engagementHeatmap(
	moments: EngagementMoment[],
	durationSec: number,
	buckets = 24,
): {
	bins: { startSec: number; reactions: number; comments: number; total: number }[];
	max: number;
	peakSec: number | null;
} {
	const span = Math.max(1, Math.floor(durationSec));
	const n = Math.max(1, buckets);
	const slice = span / n;
	const bins = Array.from({ length: n }, (_, i) => ({
		startSec: Math.round(i * slice),
		reactions: 0,
		comments: 0,
		total: 0,
	}));
	for (const m of moments) {
		const at = Math.max(0, Math.min(span - 0.001, m.atSeconds));
		const idx = Math.min(n - 1, Math.floor(at / slice));
		const bin = bins[idx]!;
		if (m.kind === "comment") bin.comments++;
		else bin.reactions++;
		bin.total++;
	}
	let max = 0;
	let peakSec: number | null = null;
	for (const b of bins) {
		if (b.total > max) {
			max = b.total;
			peakSec = b.startSec;
		}
	}
	return { bins, max, peakSec };
}
