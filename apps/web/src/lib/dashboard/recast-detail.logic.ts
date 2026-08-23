/** Pure adapters + assembly for the recast detail page. */

import {
	type Activity,
	avgWatchPct,
	completionRate,
	engagementRate,
	uniqueViewers,
	viewCount,
} from "./activity";
import { formatBytes, formatDuration, formatRelative } from "./format";
import type { Recast } from "./store.svelte";

/** The detail loader's recast shape — a superset of what the player needs. */
interface DetailRecast {
	id: string;
	title: string;
	durationSec: number;
	createdAt: number;
	sizeBytes: number;
	source: string;
	provider: string | null;
	videoUrl: string;
	posterUrl: string | null;
}

/** Adapt the loader's recast into the store-shaped `Recast` the player wants. */
export function toPlayerRecast(
	recast: DetailRecast,
	shares: { slug: string }[],
	views: number,
): Recast {
	return {
		id: recast.id,
		title: recast.title,
		durationSec: recast.durationSec,
		createdAt: recast.createdAt,
		sizeBytes: recast.sizeBytes,
		source: recast.source as Recast["source"],
		provider: recast.provider,
		views,
		folderId: null,
		tags: [],
		videoUrl: recast.videoUrl,
		posterUrl: recast.posterUrl ?? "",
		latestShareSlug: shares[0]?.slug ?? null,
	};
}

/** "4:12 · 182 MB · 2 days ago" — duration · size · age. */
export function formatRecastSubtitle(recast: {
	durationSec: number;
	sizeBytes: number;
	createdAt: number;
}): string {
	return `${formatDuration(recast.durationSec)} · ${formatBytes(recast.sizeBytes)} · ${formatRelative(recast.createdAt)}`;
}

/**
 * Lifetime stat row. Comments/Reactions are broken out in the Engagement card +
 * heatmap, so the row carries the headline rates instead. Icons are passed in
 * so this stays free of component imports.
 */
export function buildStatRow<I>(
	activity: Activity[],
	engagement: { reactionCount: number; commentCount: number },
	icons: {
		views: I;
		reach: I;
		engagement: I;
		avgWatch: I;
		completion: I;
		interactions: I;
	},
): { icon: I; label: string; value: string }[] {
	const lifetimeViews = viewCount(activity);
	const interactions = engagement.reactionCount + engagement.commentCount;
	return [
		{ icon: icons.views, label: "Views", value: String(lifetimeViews) },
		{ icon: icons.reach, label: "Reach", value: String(uniqueViewers(activity)) },
		{
			icon: icons.engagement,
			label: "Engagement",
			value: `${engagementRate(lifetimeViews, engagement.reactionCount, engagement.commentCount)}%`,
		},
		{ icon: icons.avgWatch, label: "Avg watch", value: `${avgWatchPct(activity)}%` },
		{ icon: icons.completion, label: "Completion", value: `${completionRate(activity)}%` },
		{ icon: icons.interactions, label: "Interactions", value: String(interactions) },
	];
}
