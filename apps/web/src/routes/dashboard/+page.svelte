<script lang="ts">
	import PerformanceHero from "$lib/dashboard/components/PerformanceHero.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import RecentRecasts from "$lib/dashboard/components/RecentRecasts.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import UsageMeter from "$lib/dashboard/components/UsageMeter.svelte";
	import { avgWatchPct, completionRate, uniqueViewers, viewsByDay } from "$lib/dashboard/activity";
	import { formatBytes } from "$lib/dashboard/format";
	import { mapRecastsForStore } from "$lib/dashboard/hydrate";
	import { quickUpload } from "$lib/dashboard/quick-upload.svelte";
	import { quotaStore, recastsStore, settingsStore } from "$lib/dashboard/store.svelte";
	import { Cloud, HardDrive, Users, Video } from "@recast/icons";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	// Hydrate the local store with the server-loaded list (home omits folders/tags).
	$effect(() => {
		const mapped = mapRecastsForStore(data.recasts, { folders: false, tags: false });
		const ws = data.workspaceId;
		untrack(() => recastsStore.hydrate(mapped, ws));
	});

	const firstName = $derived(settingsStore.value.profile.name.split(/\s+/)[0] ?? "there");
	const activity = $derived(data.activity);


	const totalViews = $derived(recastsStore.items.reduce((s, r) => s + r.views, 0));
	const spark14 = $derived(viewsByDay(activity, 14));
	const last7 = $derived(spark14.slice(7).reduce((s, b) => s + b.views, 0));
	const prev7 = $derived(spark14.slice(0, 7).reduce((s, b) => s + b.views, 0));
	const trendPct = $derived(prev7 > 0 ? Math.round(((last7 - prev7) / prev7) * 100) : null);
	const viewers = $derived(uniqueViewers(activity));
	const completion = $derived(completionRate(activity));
	const avgWatch = $derived(avgWatchPct(activity));

	// Secondary "library" facts — the inventory/storage side, kept distinct from
	// the hero's performance metrics so nothing is duplicated.
	const usedBytes = $derived(quotaStore.value?.usage.storageBytes ?? recastsStore.usedBytes);
	const libraryStats = $derived([
		{ icon: Video, label: "Recasts", value: String(recastsStore.items.length) },
		{ icon: Cloud, label: "On cloud", value: String(recastsStore.cloudCount) },
		{ icon: HardDrive, label: "Storage used", value: formatBytes(usedBytes) },
		{ icon: Users, label: "Team", value: String(quotaStore.value?.usage.membersCount ?? 1) },
	]);
</script>

<svelte:head>
	<title>Dashboard - Recast Dashboard</title>
</svelte:head>

<PerformanceHero
	{firstName}
	{totalViews}
	{last7}
	{trendPct}
	{viewers}
	{completion}
	{avgWatch}
	onNew={() => quickUpload.show()}
/>

<section class="mt-6" in:fly={{ y: 12, duration: 480, delay: 160, easing: cubicOut }}>
	<h2 class="mb-3 px-1 text-caption font-medium text-muted-foreground">Library</h2>
	<StatGrid stats={libraryStats} />
</section>

<!-- What's happening — the performance signal, usage rides alongside. -->
<div class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-3">
	<div class="lg:col-span-2" in:fly={{ y: 12, duration: 480, delay: 220, easing: cubicOut }}>
		<RecentActivity {activity} limit={7} />
	</div>
	<div in:fly={{ y: 12, duration: 480, delay: 280, easing: cubicOut }}>
		<UsageMeter />
	</div>
</div>

<!-- Resume / browse recent work. -->
<div class="mt-4" in:fly={{ y: 12, duration: 480, delay: 340, easing: cubicOut }}>
	<RecentRecasts recasts={recastsStore.items} />
</div>
