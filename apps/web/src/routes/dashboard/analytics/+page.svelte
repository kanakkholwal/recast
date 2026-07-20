<script lang="ts">
	import {
		avgWatchPct,
		completionRate,
		deviceBreakdown,
		geographyBreakdown,
		trafficBreakdown,
		uniqueViewers,
		viewCount,
		viewsByDay,
		watchRetention,
	} from "$lib/dashboard/activity";
	import ActivityBarChart from "$lib/dashboard/components/ActivityBarChart.svelte";
	import BreakdownList from "$lib/dashboard/components/BreakdownList.svelte";
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import RecastPerformanceTable from "$lib/dashboard/components/RecastPerformanceTable.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import WatchRetention from "$lib/dashboard/components/WatchRetention.svelte";
	import { formatCount } from "$lib/dashboard/format";
	import * as Select from "@recast/ui/select";
	import {
		BarChart3,
		CalendarDays,
		Eye,
		Globe2,
		Link2,
		MessageSquare,
		Percent,
		Smartphone,
		Target,
		Users,
	} from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	type RangeFilter = "7d" | "30d" | "all";

	let range = $state<RangeFilter>("7d");
	const rangeOptions = [
		{ label: "Last 7 days", value: "7d" },
		{ label: "Last 30 days", value: "30d" },
		{ label: "All time", value: "all" },
	] satisfies { label: string; value: RangeFilter }[];
	const days = $derived(range === "7d" ? 7 : range === "30d" ? 30 : 365);
	const rangeLabel = $derived(rangeOptions.find((o) => o.value === range)?.label ?? "Last 7 days");

	const activity = $derived(
		data.activity.filter((a) => {
			if (range === "all") return true;
			return a.timestamp >= Date.now() - days * 86_400_000;
		}),
	);

	const totalViews = $derived(viewCount(activity));
	const chartData = $derived(viewsByDay(activity, range === "7d" ? 7 : range === "30d" ? 14 : 30));
	const retention = $derived(watchRetention(activity));
	const geography = $derived(geographyBreakdown(activity));
	const devices = $derived(deviceBreakdown(activity));
	const traffic = $derived(trafficBreakdown(activity));
	const completion = $derived(completionRate(activity));
	const bestPerformer = $derived(
		data.performance.length
			? [...data.performance].sort((a, b) => b.views - a.views)[0]
			: null,
	);
	const activeRecasts = $derived(data.performance.filter((r) => r.views > 0).length);

	const stats = $derived([
		{ icon: Eye, label: "Views", value: formatCount(totalViews) },
		{ icon: Percent, label: "Avg watch", value: `${avgWatchPct(activity)}%` },
		{ icon: Users, label: "Unique viewers", value: formatCount(uniqueViewers(activity)) },
		{ icon: MessageSquare, label: "Comments", value: formatCount(data.commentsTotal) },
	]);
</script>

<svelte:head>
	<title>Analytics - Recast Dashboard</title>
</svelte:head>

<PageHeader icon={BarChart3} title="Analytics" subtitle="How your shared recasts are performing.">
	<div class="flex w-full sm:w-auto">
		<Select.Root type="single" bind:value={range}>
			<Select.Trigger
				aria-label="Filter analytics date range"
				class="h-9 w-full justify-between border-border-low/60 bg-card/40 text-xs font-semibold hover:border-border-low sm:w-40"
			>
				<span class="flex min-w-0 items-center gap-2">
					<CalendarDays class="size-4 text-muted-foreground" />
					<span class="truncate">{rangeLabel}</span>
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each rangeOptions as option (option.value)}
					<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
</PageHeader>

<div class="mt-7">
	<StatGrid {stats} />
</div>

<section
	class="mt-5 grid grid-cols-1 gap-3 md:grid-cols-3"
	in:fly={{ y: 12, duration: 480, delay: 260, easing: cubicOut }}
>
	<div class="glass-card rounded-xl p-4">
		<div class="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			<Target class="size-3.5" />
			Completion
		</div>
		<p class="mt-2 font-mono text-2xl font-semibold tabular-nums tracking-tight text-foreground">
			{completion}%
		</p>
		<p class="mt-1 text-xs leading-5 text-muted-foreground">
			Share of plays that reached the end in the selected range.
		</p>
	</div>
	<div class="glass-card rounded-xl p-4">
		<div class="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			<BarChart3 class="size-3.5" />
			Active recasts
		</div>
		<p class="mt-2 font-mono text-2xl font-semibold tabular-nums tracking-tight text-foreground">
			{formatCount(activeRecasts)}
		</p>
		<p class="mt-1 text-xs leading-5 text-muted-foreground">
			Recasts with at least one view across the workspace.
		</p>
	</div>
	<div class="glass-card rounded-xl p-4">
		<div class="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			<Eye class="size-3.5" />
			Top recast
		</div>
		<p class="mt-2 truncate text-sm font-semibold text-foreground" title={bestPerformer?.title ?? "No views yet"}>
			{bestPerformer?.title ?? "No views yet"}
		</p>
		<p class="mt-1 font-mono text-xs tabular-nums text-muted-foreground">
			{formatCount(bestPerformer?.views ?? 0)} views
		</p>
	</div>
</section>

<div class="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
	<div class="space-y-4">
		<div in:fly={{ y: 12, duration: 480, delay: 320, easing: cubicOut }}>
			<ActivityBarChart data={chartData} />
		</div>

		<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
			<div in:fly={{ y: 12, duration: 480, delay: 380, easing: cubicOut }}>
				<WatchRetention data={retention} />
			</div>
			<div in:fly={{ y: 12, duration: 480, delay: 440, easing: cubicOut }}>
				<BreakdownList title="Traffic sources" icon={Link2} rows={traffic} empty="No referrer data yet." />
			</div>
		</div>

		<div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
			<div in:fly={{ y: 12, duration: 480, delay: 500, easing: cubicOut }}>
				<BreakdownList title="Audience locations" icon={Globe2} rows={geography} empty="No location data yet." />
			</div>
			<div in:fly={{ y: 12, duration: 480, delay: 560, easing: cubicOut }}>
				<BreakdownList title="Devices" icon={Smartphone} rows={devices} empty="No device data yet." />
			</div>
		</div>

		<div in:fly={{ y: 12, duration: 480, delay: 620, easing: cubicOut }}>
			<RecastPerformanceTable rows={data.performance} />
		</div>
	</div>

	<aside class="xl:sticky xl:top-24 xl:self-start" in:fly={{ y: 12, duration: 480, delay: 680, easing: cubicOut }}>
		<RecentActivity {activity} limit={12} linkHref={null} />
	</aside>
</div>
