<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import PerformanceHero from "$lib/dashboard/components/PerformanceHero.svelte";
	import PlayerDialog from "$lib/dashboard/components/PlayerDialog.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import RecentRecasts from "$lib/dashboard/components/RecentRecasts.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import UsageMeter from "$lib/dashboard/components/UsageMeter.svelte";
	import { avgWatchPct, completionRate, uniqueViewers, viewsByDay } from "$lib/dashboard/activity";
	import { formatBytes } from "$lib/dashboard/format";
	import { mapRecastsForStore } from "$lib/dashboard/hydrate";
	import { quotaStore, recastsStore, settingsStore, type Recast } from "$lib/dashboard/store.svelte";
	import { UPLOAD_ACCEPT } from "$lib/dashboard/upload";
	import { createUploadController } from "$lib/dashboard/upload.svelte";
	import { Cloud, HardDrive, Users, Video } from "@lucide/svelte";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly, slide } from "svelte/transition";

	let { data } = $props();

	// Hydrate the local store with the server-loaded list (home omits folders/tags).
	$effect(() => {
		const mapped = mapRecastsForStore(data.recasts, { folders: false, tags: false });
		untrack(() => recastsStore.hydrate(mapped));
	});

	const workspaceId = $derived(data.workspaceId);
	const firstName = $derived(settingsStore.value.profile.name.split(/\s+/)[0] ?? "there");
	const activity = $derived(data.activity);

	// Hero — performance summary. All figures are real: lifetime total views
	// from the recast rows, and a genuine last-7-days trend from the recent
	// activity slice (no fabricated deltas).
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

	let playing = $state<Recast | null>(null);

	// Upload — same flow the library uses, so the home page is a real entry point.
	let fileInput = $state<HTMLInputElement | null>(null);
	const upload = createUploadController({
		workspaceId: () => workspaceId,
		onRefresh: invalidateAll,
	});
</script>

<svelte:head>
	<title>Home - Recast Dashboard</title>
</svelte:head>

<input bind:this={fileInput} type="file" accept={UPLOAD_ACCEPT} class="hidden" onchange={upload.onFilePicked} />

<PerformanceHero
	{firstName}
	{totalViews}
	{last7}
	{trendPct}
	{viewers}
	{completion}
	{avgWatch}
	uploading={upload.uploading}
	uploadLabel={upload.label}
	onNew={() => fileInput?.click()}
/>

{#if upload.uploading}
	<div class="mt-4" transition:slide={{ duration: 200, easing: cubicOut }}>
		<div class="flex items-center justify-between text-xs text-muted-foreground">
			<span class="font-medium text-foreground">{upload.label}</span>
			{#if upload.phase === "uploading"}<span class="font-mono tabular-nums">{upload.pct}%</span>{/if}
		</div>
		<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8">
			<div
				class="h-full rounded-full bg-linear-to-r from-primary/70 to-primary transition-[width] duration-300 ease-[cubic-bezier(0.625,0.05,0,1)]"
				style="width: {upload.phase === 'uploading' ? upload.pct : 100}%"
				class:animate-pulse={upload.phase !== "uploading"}
			></div>
		</div>
	</div>
{/if}

<!-- Library at a glance — orientation counts, kept visually subordinate to the
	 hero above but surfaced high where they're actually useful. -->
<section class="mt-6" in:fly={{ y: 12, duration: 480, delay: 160, easing: cubicOut }}>
	<h2 class="mb-3 px-1 text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
		Library
	</h2>
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
	<RecentRecasts recasts={recastsStore.items} onplay={(rec) => (playing = rec)} />
</div>

{#if playing}
	<PlayerDialog
		recast={playing}
		onclose={() => (playing = null)}
		onengagement={(event) => {
			if (event.type === "view-start" && playing) {
				recastsStore.incrementViews(playing.id);
			}
		}}
	/>
{/if}
