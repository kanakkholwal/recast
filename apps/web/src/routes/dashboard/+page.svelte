<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import PlayerDialog from "$lib/dashboard/components/PlayerDialog.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import RecentRecasts from "$lib/dashboard/components/RecentRecasts.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import UsageMeter from "$lib/dashboard/components/UsageMeter.svelte";
	import { formatBytes, formatCount } from "$lib/dashboard/format";
	import { mapRecastsForStore } from "$lib/dashboard/hydrate";
	import { quotaStore, recastsStore, settingsStore, type Recast } from "$lib/dashboard/store.svelte";
	import { UPLOAD_ACCEPT } from "$lib/dashboard/upload";
	import { createUploadController } from "$lib/dashboard/upload.svelte";
	import { BarChart3, Cloud, Eye, Film, LayoutDashboard, LoaderCircle, Upload, Video } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
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

	const totalViews = $derived(recastsStore.items.reduce((s, r) => s + r.views, 0));
	const activity = $derived(data.activity);
	const usedBytes = $derived(quotaStore.value?.usage.storageBytes ?? recastsStore.usedBytes);

	const stats = $derived([
		{ icon: Video, label: "Recasts", value: String(recastsStore.items.length) },
		{ icon: Eye, label: "Total views", value: formatCount(totalViews) },
		{ icon: Cloud, label: "On cloud", value: String(recastsStore.cloudCount) },
		{ icon: Film, label: "Storage used", value: formatBytes(usedBytes) },
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

<PageHeader icon={LayoutDashboard} title="Welcome back, {firstName}." subtitle="Here's what's happening across your recasts.">
	<Button variant="outline" size="sm" href="/dashboard/analytics" class="gap-2">
		<BarChart3 class="size-3.5" />
		Analytics
	</Button>
	<Button size="sm" class="gap-2" disabled={upload.uploading} onclick={() => fileInput?.click()}>
		{#if upload.uploading}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Upload class="size-3.5" />{/if}
		{upload.uploading ? upload.label : "Upload"}
	</Button>
</PageHeader>

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

<!-- Stats -->
<div class="mt-7">
	<StatGrid {stats} />
</div>

<!-- Recent recasts (visual rail) -->
<div class="mt-8" in:fly={{ y: 12, duration: 480, delay: 300, easing: cubicOut }}>
	<RecentRecasts recasts={recastsStore.items} onplay={(rec) => (playing = rec)} />
</div>

<!-- Activity + side stack -->
<div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
	<div class="lg:col-span-2" in:fly={{ y: 12, duration: 480, delay: 360, easing: cubicOut }}>
		<RecentActivity {activity} limit={7} />
	</div>

	<div class="flex flex-col gap-4">
		<div in:fly={{ y: 12, duration: 480, delay: 420, easing: cubicOut }}>
			<UsageMeter />
		</div>
	</div>
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
