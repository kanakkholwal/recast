<script lang="ts">
	import {
		avgWatchPct,
		generateActivity,
		uniqueViewers,
		viewsByDay,
	} from "$lib/dashboard/activity";
	import ActivityBarChart from "$lib/dashboard/components/ActivityBarChart.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import StatCard from "$lib/dashboard/components/StatCard.svelte";
	import TopRecasts from "$lib/dashboard/components/TopRecasts.svelte";
	import { formatCount } from "$lib/dashboard/format";
	import { recastsStore } from "$lib/dashboard/store.svelte";
	import { BarChart3, Eye, Percent, Share2, Users } from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	type Range = "7d" | "30d" | "all";
	let range = $state<Range>("7d");

	const ranges: { label: string; value: Range; days: number }[] = [
		{ label: "Last 7 days", value: "7d", days: 7 },
		{ label: "Last 30 days", value: "30d", days: 30 },
		{ label: "All time", value: "all", days: 365 },
	];

	const allActivity = $derived(generateActivity(recastsStore.items));
	const days = $derived(
		ranges.find((r) => r.value === range)?.days ?? 7,
	);
	const cutoff = $derived(Date.now() - days * 86_400_000);

	const activity = $derived(
		range === "all"
			? allActivity
			: allActivity.filter((a) => a.timestamp >= cutoff),
	);

	const totalViews = $derived(
		activity.filter((a) => a.kind === "viewed" || a.kind === "completed").length,
	);
	const watchPct = $derived(avgWatchPct(activity));
	const viewers = $derived(uniqueViewers(activity));
	const sharedCount = $derived(recastsStore.cloudCount);

	const chartData = $derived(viewsByDay(activity, range === "7d" ? 7 : range === "30d" ? 14 : 30));

	const stats = $derived([
		{ icon: Eye, label: "Views", value: formatCount(totalViews) },
		{ icon: Percent, label: "Avg watch", value: `${watchPct}%` },
		{ icon: Users, label: "Unique viewers", value: formatCount(viewers) },
		{ icon: Share2, label: "Shared recasts", value: String(sharedCount) },
	]);
</script>

<svelte:head>
	<title>Analytics — Recast Dashboard</title>
</svelte:head>

<header
	class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between"
	in:fly={{ y: 12, duration: 500, easing: cubicOut }}
>
	<div>
		<div class="flex items-center gap-2">
			<BarChart3 class="size-5 text-primary" />
			<h1 class="text-2xl font-semibold tracking-tight text-foreground">Analytics</h1>
		</div>
		<p class="mt-1 text-sm text-muted-foreground">
			How your shared recasts are performing.
		</p>
	</div>

	<!-- Range selector -->
	<div class="flex items-center gap-1 rounded-lg border border-border-low/60 bg-card/40 p-1">
		{#each ranges as r (r.value)}
			<button
				type="button"
				onclick={() => (range = r.value)}
				aria-pressed={range === r.value}
				class="rounded-md px-3 py-1.5 text-xs font-semibold transition-colors duration-200
					{range === r.value
					? 'bg-primary/12 text-foreground'
					: 'text-muted-foreground hover:text-foreground'}"
			>
				{r.label}
			</button>
		{/each}
	</div>
</header>

<!-- Stats -->
<div class="mt-7 grid grid-cols-2 gap-3 lg:grid-cols-4">
	{#each stats as stat, i (stat.label)}
		<div in:fly={{ y: 12, duration: 480, delay: 80 + i * 60, easing: cubicOut }}>
			<StatCard icon={stat.icon} label={stat.label} value={stat.value} />
		</div>
	{/each}
</div>

<!-- Chart + top recasts -->
<div class="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-3">
	<div
		class="lg:col-span-2"
		in:fly={{ y: 12, duration: 480, delay: 360, easing: cubicOut }}
	>
		<ActivityBarChart data={chartData} />
	</div>
	<div in:fly={{ y: 12, duration: 480, delay: 420, easing: cubicOut }}>
		<TopRecasts recasts={recastsStore.items} />
	</div>
</div>

<!-- Viewer activity -->
<div class="mt-8" in:fly={{ y: 12, duration: 480, delay: 480, easing: cubicOut }}>
	<RecentActivity {activity} limit={12} />
</div>
