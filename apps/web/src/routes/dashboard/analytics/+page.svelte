<script lang="ts">
import {
	ArrowRight,
	BarChart3,
	CheckCircle2,
	Eye,
	Globe2,
	Link2,
	Percent,
	Repeat,
	Smartphone,
	Upload,
	Users,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import {
	deviceBreakdown,
	geographyBreakdown,
	trafficBreakdown,
	viewsByDay,
	watchRetention,
} from "$lib/dashboard/activity";
import {
	deltaPct,
	dropOffPoint,
	inRange,
	peakBucket,
	periodStats,
	previousRange,
	type RangeKey,
	returningViewers,
} from "$lib/dashboard/analytics.logic";
import ActivityBarChart from "$lib/dashboard/components/ActivityBarChart.svelte";
import BreakdownList from "$lib/dashboard/components/BreakdownList.svelte";
import EmptyState from "$lib/dashboard/components/EmptyState.svelte";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
import RangeTabs from "$lib/dashboard/components/RangeTabs.svelte";
import RecastPerformanceTable from "$lib/dashboard/components/RecastPerformanceTable.svelte";
import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
import WatchRetention from "$lib/dashboard/components/WatchRetention.svelte";
import { formatCount } from "$lib/dashboard/format";

let { data } = $props();

let range = $state<RangeKey>("7d");
const rangeOptions = [
	{ label: "7 days", value: "7d" },
	{ label: "30 days", value: "30d" },
	{ label: "All time", value: "all" },
];

const activity = $derived(inRange(data.activity, range));
const prior = $derived(previousRange(data.activity, range));
const now = $derived(periodStats(activity));
const before = $derived(periodStats(prior));

const chartData = $derived(viewsByDay(activity, range === "7d" ? 7 : range === "30d" ? 14 : 30));
const peak = $derived(peakBucket(chartData));
const retention = $derived(watchRetention(activity));
const halfGone = $derived(dropOffPoint(retention));

const geography = $derived(geographyBreakdown(activity));
const devices = $derived(deviceBreakdown(activity));
const traffic = $derived(trafficBreakdown(activity));
const returning = $derived(returningViewers(activity));

// A number with nothing to compare it to can't be judged, and All time has no prior window, so it shows none.
const stats = $derived([
	{
		icon: Eye,
		label: "Views",
		value: formatCount(now.views),
		delta: deltaPct(now.views, before.views),
	},
	{
		icon: Users,
		label: "Unique viewers",
		value: formatCount(now.viewers),
		delta: deltaPct(now.viewers, before.viewers),
	},
	{
		icon: Percent,
		label: "Avg watch",
		value: `${now.avgWatch}%`,
		delta: deltaPct(now.avgWatch, before.avgWatch),
	},
	{
		icon: CheckCircle2,
		label: "Completion",
		value: `${now.completion}%`,
		delta: deltaPct(now.completion, before.completion),
	},
]);

// Nothing has ever been viewed: eleven zeroed panels help no one.
const noData = $derived(data.activity.length === 0);
const emptyRange = $derived(!noData && now.views === 0);
</script>

<svelte:head>
	<title>Analytics - Recast Dashboard</title>
</svelte:head>

<PageHeader icon={BarChart3} title="Analytics" subtitle="How your shared recasts are performing.">
	{#if !noData}
		<RangeTabs bind:value={range} options={rangeOptions} />
	{/if}
</PageHeader>

{#if noData}
	<div class="surface mt-6" in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}>
		<EmptyState
			bordered={false}
			icon={BarChart3}
			title="No views yet"
			description="Share a recast and views, watch time and audience land here within minutes."
		>
			<div class="flex flex-wrap items-center justify-center gap-3">
				<Button href="/dashboard/recasts" variant="dark" class="gap-2">
					<Upload class="size-4" />
					Go to your library
				</Button>
				<Button href="/dashboard" variant="outline" class="group/cta gap-2">
					Dashboard
					<ArrowRight
						class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
					/>
				</Button>
			</div>
		</EmptyState>
	</div>
{:else}
	<div class="mt-6" in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}>
		<StatGrid {stats} class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4" />
	</div>

	{#if emptyRange}
		<p class="mt-3 text-body-sm text-muted-foreground">
			No views in this range. Switch to All time to see earlier activity.
		</p>
	{/if}

	<!-- Trend, with the peak called out so the chart states its own headline. -->
	<section class="mt-4" in:fly={{ y: 12, duration: 480, delay: 140, easing: cubicOut }}>
		<div class="mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
			<h2 class="font-display text-body font-medium text-foreground">Views over time</h2>
			{#if peak}
				<p class="text-body-sm text-muted-foreground">
					Busiest: <span class="font-medium text-foreground">{peak.label}</span>, {formatCount(
						peak.views,
					)} views
				</p>
			{/if}
		</div>
		<ActivityBarChart data={chartData} />
	</section>

	<!-- Retention answers "do they finish", audience answers "who are they". -->
	<section class="mt-4" in:fly={{ y: 12, duration: 480, delay: 200, easing: cubicOut }}>
		<div class="mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
			<h2 class="font-display text-body font-medium text-foreground">Attention</h2>
			{#if halfGone !== null}
				<p class="text-body-sm text-muted-foreground">
					Half your viewers leave by <span class="font-medium text-foreground">{halfGone}%</span> of
					the video
				</p>
			{/if}
		</div>
		<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
			<WatchRetention data={retention} />
			<BreakdownList
				title="Traffic sources"
				icon={Link2}
				rows={traffic}
				empty="No referrer data yet."
			/>
		</div>
	</section>

	<section class="mt-6" in:fly={{ y: 12, duration: 480, delay: 260, easing: cubicOut }}>
		<h2 class="mb-3 font-display text-body font-medium text-foreground">Audience</h2>
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
			<BreakdownList
				title="Locations"
				icon={Globe2}
				rows={geography}
				empty="No location data yet."
			/>
			<BreakdownList
				title="Devices"
				icon={Smartphone}
				rows={devices}
				empty="No device data yet."
			/>

			<!-- The one thing a view count can't tell you: did anyone come back. -->
			<section class="surface flex h-full flex-col p-5">
				<header class="flex items-center gap-2">
					<Repeat class="size-4 text-muted-foreground" />
					<h3 class="font-display text-body font-medium text-foreground">Returning viewers</h3>
				</header>
				{#if now.viewers === 0}
					<p class="mt-4 text-body-sm text-muted-foreground">No viewers in this range yet.</p>
				{:else}
					<p class="mt-4 text-heading-sm font-display font-medium tabular-nums text-foreground">
						{formatCount(returning.count)}
					</p>
					<p class="mt-1 text-body-sm text-muted-foreground">
						of {formatCount(now.viewers)} watched on more than one day
					</p>
					<div
						class="mt-4 h-1.5 overflow-hidden rounded-full bg-paper"
						role="progressbar"
						aria-label="Share of viewers who returned"
						aria-valuenow={returning.pct}
						aria-valuemin={0}
						aria-valuemax={100}
					>
						<div
							class="h-full rounded-full bg-foreground transition-[width] duration-500 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none"
							style="width: {returning.pct > 0 && returning.pct < 2 ? 2 : returning.pct}%"
						></div>
					</div>
					<p class="mt-1.5 text-caption tabular-nums text-muted-foreground">
						{returning.pct}% came back
					</p>
				{/if}
			</section>
		</div>
	</section>

	<!-- Lifetime by construction: comments only exist as an all-time rollup, so
	     the table says so rather than inheriting the range filter's label. -->
	<section class="mt-6" in:fly={{ y: 12, duration: 480, delay: 320, easing: cubicOut }}>
		<div class="mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
			<h2 class="font-display text-body font-medium text-foreground">Recast performance</h2>
			<p class="text-body-sm text-muted-foreground">All time, not the selected range</p>
		</div>
		<RecastPerformanceTable rows={data.performance} />
	</section>

	<section class="mt-4" in:fly={{ y: 12, duration: 480, delay: 380, easing: cubicOut }}>
		<RecentActivity {activity} limit={8} linkHref={null} />
	</section>
{/if}
