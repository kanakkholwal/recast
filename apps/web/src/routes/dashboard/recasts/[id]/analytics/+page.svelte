<script lang="ts">
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
	import RangeTabs from "$lib/dashboard/components/RangeTabs.svelte";
	import RecastEngagement from "$lib/dashboard/components/RecastEngagement.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import WatchRetention from "$lib/dashboard/components/WatchRetention.svelte";
	import { buildStatRow } from "$lib/dashboard/recast-detail.logic";
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

	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();

	const recast = $derived(data.recast);

	// ── Range (chart + retention only; the stat row is lifetime) ────────
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
	const chartData = $derived(viewsByDay(ranged, range === "7d" ? 7 : range === "30d" ? 14 : 30));
	const retention = $derived(watchRetention(ranged));

	// ── Lifetime stats (Comments/Reactions are broken out in the Engagement
	//    card + heatmap, so the row carries the headline rates instead). ──────
	const stats = $derived(
		buildStatRow(data.activity, data.engagement, {
			views: Eye,
			reach: Users,
			engagement: Zap,
			avgWatch: Percent,
			completion: CheckCircle2,
			interactions: MessageSquare,
		}),
	);

	// ── Audience breakdowns (computed from the already-loaded activity) ──────
	const geography = $derived(geographyBreakdown(data.activity));
	const devices = $derived(deviceBreakdown(data.activity));
	const traffic = $derived(trafficBreakdown(data.activity));
</script>

<svelte:head>
	<title>{recast.title} · Analytics - Recast</title>
</svelte:head>

<p class="mb-4 flex items-center gap-1.5 text-xs text-muted-foreground">
	<Share2 class="size-3.5 shrink-0" />
	Combined across every share link for this recast.
</p>

<!-- Lifetime stats -->
<StatGrid {stats} class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6" />

<!-- Views over time -->
<section class="mt-6">
	<div class="mb-3 flex items-center justify-between">
		<h2 class="text-sm font-semibold text-foreground">Views over time</h2>
		<RangeTabs bind:value={range} options={rangeOptions} />
	</div>
	<ActivityBarChart data={chartData} />
</section>

<!-- What moments viewers reacted to -->
<div class="mt-6">
	<EngagementHeatmap moments={data.engagement.moments} durationSec={recast.durationSec} />
</div>

<!-- Retention + engagement -->
<div class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
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
