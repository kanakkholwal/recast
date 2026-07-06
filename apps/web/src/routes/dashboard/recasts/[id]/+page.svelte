<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import * as api from "$lib/dashboard/api";
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
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import RangeTabs from "$lib/dashboard/components/RangeTabs.svelte";
	import RecastEngagement from "$lib/dashboard/components/RecastEngagement.svelte";
	import RecastShares, { type ShareRow } from "$lib/dashboard/components/RecastShares.svelte";
	import RecentActivity from "$lib/dashboard/components/RecentActivity.svelte";
	import StatGrid from "$lib/dashboard/components/StatGrid.svelte";
	import WatchRetention from "$lib/dashboard/components/WatchRetention.svelte";
	import { POSTER_ACCEPT, replacePoster } from "$lib/dashboard/poster";
	import { buildStatRow, formatRecastSubtitle } from "$lib/dashboard/recast-detail.logic";
	import { RecastPlayer } from "@recast/player";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import {
		ArrowLeft,
		Check,
		CheckCircle2,
		Copy,
		Eye,
		Globe,
		ImagePlus,
		Link2,
		Loader2,
		MessageSquare,
		Percent,
		Smartphone,
		Users,
		Zap,
	} from "@lucide/svelte";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	const recast = $derived(data.recast);

	// Local share list so revoke/create reflect immediately, re-seeded from the
	// loader on navigation / invalidateAll.
	let shares = $state<ShareRow[]>([]);
	$effect(() => {
		const next = data.shares;
		untrack(() => (shares = next));
	});

	let creatingShare = $state(false);

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

	const subtitle = $derived(formatRecastSubtitle(recast));

	// Copy-link feedback lives on the button itself (tick + "Copied link")
	// rather than a toast, so the action confirms right where it happened.
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	async function copyLink() {
		try {
			let slug = shares[0]?.slug ?? null;
			if (!slug) {
				const { slug: newSlug } = await api.shareRecast(recast.id);
				slug = newSlug;
				await invalidateAll();
			}
			await navigator.clipboard.writeText(`${location.origin}/share/${slug}`);
			copied = true;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = false), 2000);
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't copy the link.");
		}
	}

	async function newShare() {
		if (creatingShare) return;
		creatingShare = true;
		try {
			await api.shareRecast(recast.id);
			await invalidateAll();
			toast.success("New share link created.");
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't create a link.");
		} finally {
			creatingShare = false;
		}
	}

	async function revokeShare(slug: string) {
		const snapshot = shares;
		shares = shares.filter((s) => s.slug !== slug);
		try {
			await api.deleteShare(slug);
			toast.success("Share link revoked.");
		} catch (e) {
			shares = snapshot;
			toast.error((e as Error)?.message ?? "Couldn't revoke the link.");
		}
	}

	// ── Replace cover ───────────────────────────────────────────────────
	let posterInput = $state<HTMLInputElement | null>(null);
	let replacingPoster = $state(false);

	function pickPoster() {
		if (replacingPoster) return;
		posterInput?.click();
	}

	async function onPosterPick(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = ""; // allow re-picking the same file later
		if (!file) return;
		replacingPoster = true;
		try {
			await replacePoster(recast.id, file);
			// Re-run the loader so the (re-signed) poster URL flows back in.
			await invalidateAll();
			toast.success("Cover updated.");
		} catch (err) {
			toast.error((err as Error)?.message ?? "Couldn't update the cover.");
		} finally {
			replacingPoster = false;
		}
	}
</script>

<svelte:head>
	<title>{recast.title} - Recast</title>
</svelte:head>

<a
	href="/dashboard/recasts"
	class="mb-4 inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
>
	<ArrowLeft class="size-3.5" />
	Library
</a>

<PageHeader title={recast.title} {subtitle}>
	<Button variant="outline" size="sm" class="gap-2" disabled={replacingPoster} onclick={pickPoster}>
		{#if replacingPoster}
			<Loader2 class="size-3.5 animate-spin" />
			Saving…
		{:else}
			<ImagePlus class="size-3.5" />
			Change cover
		{/if}
	</Button>
	<Button variant="outline" size="sm" class="gap-2" onclick={copyLink}>
		{#if copied}
			<Check class="size-3.5 text-success" />
			Copied link
		{:else}
			<Copy class="size-3.5" />
			Copy link
		{/if}
	</Button>
</PageHeader>

<input
	bind:this={posterInput}
	type="file"
	accept={POSTER_ACCEPT}
	class="hidden"
	onchange={onPosterPick}
/>

<!-- Player — our RecastPlayer, playing inline so it feels native to the app. -->
<div class="mt-6" in:fly={{ y: 12, duration: 480, easing: cubicOut }}>
	<RecastPlayer
		src={recast.videoUrl}
		poster={recast.posterUrl || null}
		title={recast.title}
		aspectRatio={recast.width && recast.height ? `${recast.width} / ${recast.height}` : "16 / 9"}
		features={{ share: false }}
		className="w-full overflow-hidden rounded-2xl ring-1 ring-inset ring-border-low/40"
	/>
</div>

<!-- Lifetime stats -->
<div class="mt-6">
	<StatGrid {stats} class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6" />
</div>

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

<!-- Share links -->
<div class="mt-4">
	<RecastShares {shares} creating={creatingShare} onnew={newShare} onrevoke={revokeShare} />
</div>

<!-- Activity feed (this recast) -->
<div class="mt-4">
	<RecentActivity activity={data.activity} limit={12} linkHref={null} />
</div>
