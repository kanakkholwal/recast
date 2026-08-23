<script lang="ts">
import {
	CheckCircle2,
	Eye,
	Globe,
	Link2,
	MessageSquare,
	Percent,
	Share2,
	Smartphone,
	Users,
	Zap,
} from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import {
	deviceBreakdown,
	geographyBreakdown,
	trafficBreakdown,
	viewsByDay,
	watchRetention,
} from "$lib/dashboard/activity";
import ActivityBarChart from "$lib/dashboard/components/ActivityBarChart.svelte";
import BreakdownList, { flagEmoji } from "$lib/dashboard/components/BreakdownList.svelte";
import EngagementHeatmap from "$lib/dashboard/components/EngagementHeatmap.svelte";
import LockedPanel from "$lib/dashboard/components/LockedPanel.svelte";
import RangeTabs from "$lib/dashboard/components/RangeTabs.svelte";
import RecastEngagement from "$lib/dashboard/components/RecastEngagement.svelte";
import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
import SkeletonPreview from "$lib/dashboard/components/SkeletonPreview.svelte";
import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
import WatchRetention from "$lib/dashboard/components/WatchRetention.svelte";
import { buildStatRow } from "$lib/dashboard/recast-detail.logic";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

const recast = $derived(data.recast);
const unlocked = $derived(data.analyticsUnlocked);

// --- Range (chart + retention only; the stat row is lifetime) ---
type Range = "7d" | "30d" | "all";
let range = $state<Range>("7d");
const rangeOptions = [
	{ label: "7 days", value: "7d" },
	{ label: "30 days", value: "30d" },
	{ label: "All", value: "all" },
];
const rangeDays = $derived(range === "7d" ? 7 : range === "30d" ? 30 : 365);
const ranged = $derived(
	range === "all"
		? data.activity
		: data.activity.filter((a) => a.timestamp >= Date.now() - rangeDays * 86_400_000),
);
const chartData = $derived(
	unlocked
		? viewsByDay(ranged, range === "7d" ? 7 : range === "30d" ? 14 : 30)
		: (data.basic?.byDay ?? []),
);
const retention = $derived(watchRetention(ranged));

// --- Lifetime stats. Free reads two aggregates; Pro reads the full row. ---
const stats = $derived(
	unlocked && data.engagement
		? buildStatRow(data.activity, data.engagement, {
				views: Eye,
				reach: Users,
				engagement: Zap,
				avgWatch: Percent,
				completion: CheckCircle2,
				interactions: MessageSquare,
			})
		: [
				{ icon: Eye, label: "Views", value: String(data.basic?.views ?? 0) },
				{ icon: Users, label: "Unique viewers", value: String(data.basic?.viewers ?? 0) },
				{
					icon: CheckCircle2,
					label: "Completion",
					value: `${data.basic?.completionPct ?? 0}%`,
				},
			],
);

// --- Audience breakdowns (computed from the already-loaded activity) ---
const geography = $derived(geographyBreakdown(data.activity));
const devices = $derived(deviceBreakdown(data.activity));
const traffic = $derived(trafficBreakdown(data.activity));
</script>

<svelte:head>
	<title>{recast.title} · Analytics - Recast</title>
</svelte:head>

<p class="mb-4 flex items-center gap-1.5 text-body-sm text-muted-foreground">
	<Share2 class="size-3.5 shrink-0" />
	Combined across every share link for this recast.
</p>

<StatGrid
	{stats}
	class={unlocked
		? "grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6"
		: "grid grid-cols-1 gap-3 sm:grid-cols-3"}
/>

<!-- Views over time -->
<section class="mt-6" in:fly={{ y: 12, duration: 460, delay: 80, easing: cubicOut }}>
	<div class="mb-3 flex items-center justify-between gap-4">
		<h2 class="font-display text-body font-medium text-foreground">Views over time</h2>
		{#if unlocked}
			<RangeTabs bind:value={range} options={rangeOptions} />
		{:else}
			<span class="text-body-sm text-muted-foreground">Last 7 days</span>
		{/if}
	</div>
	<ActivityBarChart data={chartData} />
</section>

{#if unlocked && data.engagement}
	<!-- What moments viewers reacted to -->
	<div class="mt-6" in:fly={{ y: 12, duration: 460, delay: 140, easing: cubicOut }}>
		<EngagementHeatmap moments={data.engagement.moments} durationSec={recast.durationSec} />
	</div>

	<!-- Retention + engagement -->
	<div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
		<WatchRetention data={retention} />
		<RecastEngagement engagement={data.engagement} />
	</div>

	<!-- Audience: where from + what device + how they got here -->
	<div class="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
		<BreakdownList
			title="Top locations"
			icon={Globe}
			rows={geography}
			empty="No location data yet."
			glyph={(r) => flagEmoji(r.key)}
		/>
		<BreakdownList title="Devices" icon={Smartphone} rows={devices} empty="No device data yet." />
		<BreakdownList title="Traffic sources" icon={Link2} rows={traffic} empty="No referrer data yet." />
	</div>

	<!-- Activity feed (this recast) -->
	<div class="mt-4">
		<RecentActivity activity={data.activity} limit={12} linkHref={null} />
	</div>
{:else}
	<!-- Locked. None of this data is queried for a free workspace, so the panels
	     preview the shape of the report, never stand-in numbers. -->
	<div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
		<LockedPanel
			title="Watch retention"
			description="See exactly where viewers drop off, decile by decile."
		>
			{#snippet preview()}
				<SkeletonPreview kind="curve" />
			{/snippet}
		</LockedPanel>
		<LockedPanel
			title="Reactions and comments"
			description="Every reaction and comment, pinned to the second it happened."
		>
			{#snippet preview()}
				<SkeletonPreview kind="list" />
			{/snippet}
		</LockedPanel>
	</div>

	<div class="mt-4">
		<LockedPanel
			title="Audience breakdown"
			description="Where viewers are, what they watch on, and which link sent them."
		>
			{#snippet preview()}
				<div class="grid grid-cols-1 gap-6 sm:grid-cols-3">
					<SkeletonPreview kind="list" />
					<SkeletonPreview kind="list" />
					<SkeletonPreview kind="list" />
				</div>
			{/snippet}
		</LockedPanel>
	</div>
{/if}
