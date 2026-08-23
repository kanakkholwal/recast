<script lang="ts">
import { ArrowRight, Cloud, Share2, Upload, Users, Video, Wand2 } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { avgWatchPct, completionRate, uniqueViewers, viewsByDay } from "$lib/dashboard/activity";
import PerformanceHero from "$lib/dashboard/components/PerformanceHero.svelte";
import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
import RecentRecasts from "$lib/dashboard/components/RecentRecasts.svelte";
import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
import TopRecasts from "$lib/dashboard/components/TopRecasts.svelte";
import UsageMeter from "$lib/dashboard/components/UsageMeter.svelte";
import { mapRecastsForStore } from "$lib/dashboard/hydrate";
import { quickUpload } from "$lib/dashboard/quick-upload.svelte";
import { quotaStore, recastsStore, settingsStore } from "$lib/dashboard/store.svelte";

let { data } = $props();

// Hydrate the local store with the server-loaded list (home omits folders/tags).
$effect(() => {
	const mapped = mapRecastsForStore(data.recasts, { folders: false, tags: false });
	const ws = data.workspaceId;
	untrack(() => recastsStore.hydrate(mapped, ws));
});

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

// An empty workspace gets one instruction, not five empty panels. Storage,
// activity and rankings all read zero on day one and bury the only action
// that matters.
const isEmpty = $derived(recastsStore.items.length === 0);

// Library facts. Storage lives in the usage meter, with its bar and its cap —
// a second bare number here said the same thing with less.
const libraryStats = $derived([
	{ icon: Video, label: "Recasts", value: String(recastsStore.items.length) },
	{ icon: Cloud, label: "On cloud", value: String(recastsStore.cloudCount) },
	{ icon: Users, label: "Team", value: String(quotaStore.value?.usage.membersCount ?? 1) },
]);

const firstSteps = [
	{
		icon: Upload,
		title: "Upload a recording",
		description: "Drop in an MP4, or export straight from the desktop app.",
	},
	{
		icon: Wand2,
		title: "Let auto-polish run",
		description: "Zoom, cursor smoothing and silence cuts apply on the way in.",
	},
	{
		icon: Share2,
		title: "Share the link",
		description: "Views, completion and watch time land on this page.",
	},
];
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

{#if isEmpty}
	<section
		class="surface mt-6 p-6 sm:p-8"
		in:fly={{ y: 12, duration: 480, delay: 160, easing: cubicOut }}
	>
		<h2 class="font-display font-medium text-heading-sm text-foreground">Start with one recording</h2>
		<p class="mt-2 max-w-md text-body-sm text-muted-foreground">
			Everything on this page fills in from your first recast: usage, viewer activity and what
			people actually watch.
		</p>

		<ol class="mt-6 divide-y divide-border-low border-y border-border-low">
			{#each firstSteps as step, i (step.title)}
				{@const Icon = step.icon}
				<li class="flex gap-4 py-4">
					<Icon class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
					<div class="min-w-0">
						<h3 class="font-display text-body font-medium text-foreground">
							{i + 1}. {step.title}
						</h3>
						<p class="mt-1 text-body-sm text-muted-foreground">{step.description}</p>
					</div>
				</li>
			{/each}
		</ol>

		<div class="mt-6 flex flex-wrap items-center gap-3">
			<Button variant="dark" class="gap-2" onclick={() => quickUpload.show()}>
				<Upload class="size-4" />
				Upload a recast
			</Button>
			<Button href="/download" variant="outline" class="group/cta gap-2">
				Get the desktop app
				<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
			</Button>
		</div>
	</section>
{:else}
	<!-- Library at a glance — orientation counts, kept visually subordinate to the
		 hero above but surfaced high where they're actually useful. -->
	<section class="mt-6" in:fly={{ y: 12, duration: 480, delay: 160, easing: cubicOut }}>
		<h2 class="mb-3 px-1 text-caption font-medium text-muted-foreground">Library</h2>
		<StatGrid stats={libraryStats} class="grid grid-cols-2 gap-3 lg:grid-cols-3" />
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

	<!-- Resume recent work, and see what is actually landing. -->
	<div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
		<div class="lg:col-span-2" in:fly={{ y: 12, duration: 480, delay: 340, easing: cubicOut }}>
			<RecentRecasts recasts={recastsStore.items} />
		</div>
		<div in:fly={{ y: 12, duration: 480, delay: 400, easing: cubicOut }}>
			<TopRecasts recasts={recastsStore.items} />
		</div>
	</div>
{/if}
