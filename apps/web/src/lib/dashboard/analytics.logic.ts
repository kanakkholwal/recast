/**
 * View-model for the workspace analytics page. Pure: the page holds only the
 * selected range. Everything here derives from the activity list already
 * loaded, so period comparison costs no extra query.
 */

import { type Activity, avgWatchPct, completionRate, uniqueViewers, viewCount } from "./activity";

export type RangeKey = "7d" | "30d" | "all";

export const RANGE_DAYS: Record<Exclude<RangeKey, "all">, number> = { "7d": 7, "30d": 30 };

const DAY = 86_400_000;

/** Events inside the selected window. "all" is genuinely unbounded. */
export function inRange(activity: Activity[], range: RangeKey, now = Date.now()): Activity[] {
	if (range === "all") return activity;
	const from = now - RANGE_DAYS[range] * DAY;
	return activity.filter((a) => a.timestamp >= from);
}

/**
 * The window immediately before the selected one, same length. Empty for
 * "all", which has no prior period to compare against.
 */
export function previousRange(activity: Activity[], range: RangeKey, now = Date.now()): Activity[] {
	if (range === "all") return [];
	const span = RANGE_DAYS[range] * DAY;
	return activity.filter((a) => a.timestamp >= now - span * 2 && a.timestamp < now - span);
}

export type PeriodStats = {
	views: number;
	viewers: number;
	avgWatch: number;
	completion: number;
};

export function periodStats(activity: Activity[]): PeriodStats {
	return {
		views: viewCount(activity),
		viewers: uniqueViewers(activity),
		avgWatch: avgWatchPct(activity),
		completion: completionRate(activity),
	};
}

/**
 * Percentage change against the previous period. `null` when there is no
 * baseline: a bare "+100%" off a single view is noise, not a signal.
 */
export function deltaPct(current: number, previous: number): number | null {
	if (previous <= 0) return null;
	return Math.round(((current - previous) / previous) * 100);
}

/** Viewers who came back on a later day. The signal a one-off view isn't. */
export function returningViewers(activity: Activity[]): { count: number; pct: number } {
	const days = new Map<string, Set<number>>();
	for (const a of activity) {
		if (a.kind !== "viewed" && a.kind !== "completed") continue;
		const key = a.sessionId ?? a.viewer;
		const day = Math.floor(a.timestamp / DAY);
		const seen = days.get(key);
		if (seen) seen.add(day);
		else days.set(key, new Set([day]));
	}
	if (days.size === 0) return { count: 0, pct: 0 };
	let count = 0;
	for (const seen of days.values()) if (seen.size > 1) count++;
	return { count, pct: Math.round((count / days.size) * 100) };
}

/**
 * How far in the video half the viewers have left, read off the retention
 * curve. "Half drop off by 40%" is the sentence the curve is drawn to answer.
 */
export function dropOffPoint(curve: { pct: number; reached: number }[]): number | null {
	const first = curve.find((p) => p.reached < 50);
	return first ? first.pct : null;
}

/** Busiest bucket in the chart, for the "your best day" annotation. */
export function peakBucket<T extends { label: string; views: number }>(buckets: T[]): T | null {
	let best: T | null = null;
	for (const b of buckets) {
		if (b.views > 0 && (best === null || b.views > best.views)) best = b;
	}
	return best;
}
