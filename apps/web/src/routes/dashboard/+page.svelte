<script lang="ts">
	import { generateActivity } from "$lib/dashboard/activity";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import StatCard from "$lib/dashboard/components/StatCard.svelte";
	import TopRecasts from "$lib/dashboard/components/TopRecasts.svelte";
	import UsageMeter from "$lib/dashboard/components/UsageMeter.svelte";
	import { formatBytes, formatCount } from "$lib/dashboard/format";
	import { recastsStore, settingsStore } from "$lib/dashboard/store.svelte";
	import { ArrowRight, BarChart3, Cloud, Eye, Film, Video } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const firstName = $derived(
		settingsStore.value.profile.name.split(/\s+/)[0] ?? "there",
	);

	const totalViews = $derived(
		recastsStore.items.reduce((s, r) => s + r.views, 0),
	);

	const activity = $derived(generateActivity(recastsStore.items));

	const stats = $derived([
		{ icon: Video, label: "Recasts", value: String(recastsStore.items.length) },
		{ icon: Eye, label: "Total views", value: formatCount(totalViews) },
		{ icon: Cloud, label: "On cloud", value: String(recastsStore.cloudCount) },
		{
			icon: Film,
			label: "Storage used",
			value: formatBytes(recastsStore.usedBytes),
		},
	]);
</script>

<svelte:head>
	<title>Home - Recast Dashboard</title>
</svelte:head>

<!-- Welcome -->
<header
	class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between"
	in:fly={{ y: 12, duration: 500, easing: cubicOut }}
>
	<div>
		<h1 class="text-2xl font-semibold tracking-tight text-foreground">
			Welcome back, {firstName}.
		</h1>
		<p class="mt-1 text-sm text-muted-foreground">
			Here's what's happening across your recasts.
		</p>
	</div>
	<div class="flex items-center gap-2">
		<Button href="/dashboard/analytics" variant="outline" size="sm" class="gap-2">
			<BarChart3 class="size-3.5" />
			Analytics
		</Button>
		<Button href="/dashboard/recasts" size="sm" class="group/cta gap-2">
			View recasts
			<ArrowRight class="size-3.5 transition-transform group-hover/cta:translate-x-0.5" />
		</Button>
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

<!-- Activity + side stack -->
<div class="mt-8 grid grid-cols-1 gap-4 lg:grid-cols-3">
	<div
		class="lg:col-span-2"
		in:fly={{ y: 12, duration: 480, delay: 360, easing: cubicOut }}
	>
		<RecentActivity {activity} limit={7} />
	</div>

	<div class="flex flex-col gap-4">
		<div in:fly={{ y: 12, duration: 480, delay: 420, easing: cubicOut }}>
			<TopRecasts recasts={recastsStore.items} />
		</div>
		<div in:fly={{ y: 12, duration: 480, delay: 480, easing: cubicOut }}>
			<UsageMeter />
		</div>
	</div>
</div>
