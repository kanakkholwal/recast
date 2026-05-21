/**
 * Deterministic mock activity generator. Until a real backend records viewer
 * events, we synthesise them from each cloud recast's `views` count so the
 * Home and Analytics surfaces show plausible, stable data — the same seed
 * always produces the same events, so the UI doesn't flicker on every render.
 */

import type { Recast } from "$lib/dashboard/store.svelte";

export type ActivityKind = "viewed" | "completed" | "shared" | "downloaded";

export type Activity = {
	id: string;
	recastId: string;
	recastTitle: string;
	viewer: string;
	kind: ActivityKind;
	timestamp: number;
	watchPct: number;
};

const VIEWERS = [
	"Jane Founder",
	"Alex Chen",
	"Sam Patel",
	"Priya Mehta",
	"Robin Okafor",
	"Sasha Lee",
	"Mira Iyer",
	"Dmitri V.",
	"Noah Park",
	"Yusuf K.",
];

const HOUR = 3_600_000;
const DAY = 86_400_000;

function hash(s: string): number {
	let h = 0;
	for (let i = 0; i < s.length; i++) h = (h << 5) - h + s.charCodeAt(i);
	return h | 0;
}

/** mulberry32 — tiny, deterministic, sufficient for visual variety. */
function rng(seed: number) {
	let s = seed;
	return () => {
		s = (s + 0x6d2b79f5) | 0;
		let t = s;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

export function generateActivity(recasts: Recast[]): Activity[] {
	const out: Activity[] = [];
	const now = Date.now();
	for (const rec of recasts) {
		if (rec.source !== "cloud" || rec.views <= 0) continue;
		const r = rng(hash(rec.id));
		const count = Math.min(rec.views, 14);
		for (let i = 0; i < count; i++) {
			const ageHours = Math.floor(r() * 30 * 24);
			const viewer = VIEWERS[Math.floor(r() * VIEWERS.length)]!;
			const completed = r() > 0.55;
			const watchPct = completed ? 100 : Math.floor(20 + r() * 70);
			out.push({
				id: `${rec.id}-act-${i}`,
				recastId: rec.id,
				recastTitle: rec.title,
				viewer,
				kind: completed ? "completed" : "viewed",
				timestamp: now - ageHours * HOUR,
				watchPct,
			});
		}
		// One "shared" event per cloud recast — when the link was created.
		out.push({
			id: `${rec.id}-shared`,
			recastId: rec.id,
			recastTitle: rec.title,
			viewer: "You",
			kind: "shared",
			timestamp: rec.createdAt,
			watchPct: 0,
		});
	}
	out.sort((a, b) => b.timestamp - a.timestamp);
	return out;
}

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

export function uniqueViewers(activity: Activity[]): number {
	const set = new Set<string>();
	for (const a of activity) if (a.viewer !== "You") set.add(a.viewer);
	return set.size;
}
