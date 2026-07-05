<script lang="ts">
	import SearchTrigger from "$lib/dashboard/components/SearchTrigger.svelte";
	import { formatCount } from "$lib/dashboard/format";
	import { Button } from "@recast/ui/button";
	import { Kbd } from "@recast/ui/kbd";
	import {
		ArrowDownRight,
		ArrowUpRight,
		BarChart3,
		LoaderCircle,
		Upload,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	// Centered, desktop-inspired home hero: a big greeting, one prominent search
	// bar as the focal point (opens the shared command palette), and the
	// performance figures demoted to a quiet strip beneath.
	let {
		firstName,
		totalViews,
		last7,
		trendPct,
		viewers,
		completion,
		avgWatch,
		uploading = false,
		uploadLabel = "Upload",
		onNew,
	}: {
		firstName: string;
		totalViews: number;
		/** Views in the last 7 days — labels the trend chip. */
		last7: number;
		/** Week-over-week % change, or null when there's no prior baseline. */
		trendPct: number | null;
		viewers: number;
		completion: number;
		avgWatch: number;
		uploading?: boolean;
		uploadLabel?: string;
		onNew: () => void;
	} = $props();

	const trendUp = $derived((trendPct ?? 0) >= 0);

	// No views ever recorded — a wall of zeros is a deflating focal point, so the
	// metric strip gives way to an encouraging nudge instead.
	const noViews = $derived(totalViews === 0 && last7 === 0);
	const subtitle = $derived(
		noViews
			? "Share a recast to start tracking views and engagement."
			: "Here's how your recasts are performing.",
	);

	const metrics = $derived([
		{ value: formatCount(totalViews), label: "Total views" },
		{ value: formatCount(viewers), label: "Viewers" },
		{ value: `${completion}%`, label: "Completion" },
		{ value: `${avgWatch}%`, label: "Avg watch" },
	]);
</script>

<section
	class="flex flex-col items-center gap-6 py-4 text-center sm:py-6"
	in:fly={{ y: 12, duration: 500, easing: cubicOut }}
>
	<!-- Greeting -->
	<div class="flex flex-col items-center gap-2">
		<h1 class="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
			<span class="bg-linear-to-r from-foreground to-foreground/60 bg-clip-text text-transparent">
				Welcome back, {firstName}.
			</span>
		</h1>
		<p class="max-w-md text-sm leading-relaxed text-muted-foreground">
			{subtitle} Press
			<Kbd class="mx-0.5 align-middle">
				<span class="text-[8px] font-semibold">⌘</span>
				<span class="text-[10px]">K</span>
			</Kbd>
			to search anywhere.
		</p>
	</div>

	<!-- Focal search — this route surfaces it here instead of the header. -->
	<div class="w-full max-w-xl" in:fly={{ y: 12, duration: 500, delay: 60, easing: cubicOut }}>
		<SearchTrigger variant="hero" />
	</div>

	<!-- Actions -->
	<div class="flex items-center gap-2" in:fly={{ y: 12, duration: 500, delay: 120, easing: cubicOut }}>
		<Button variant="outline" size="sm" href="/dashboard/analytics" class="gap-2">
			<BarChart3 class="size-3.5" />
			Analytics
		</Button>
		<Button size="sm" class="gap-2" disabled={uploading} onclick={onNew}>
			{#if uploading}
				<LoaderCircle class="size-3.5 animate-spin" />
			{:else}
				<Upload class="size-3.5" />
			{/if}
			{uploading ? uploadLabel : "Upload"}
		</Button>
	</div>

	<!-- Demoted performance strip -->
	{#if noViews}
		<p class="max-w-sm text-xs text-muted-foreground/80">
			No views yet — share a recast and its views, completion, and watch time land here.
		</p>
	{:else}
		<div class="flex flex-wrap items-start justify-center gap-x-8 gap-y-4">
			{#each metrics as m (m.label)}
				<div class="flex flex-col items-center">
					<span class="text-xl font-semibold tabular-nums text-foreground">{m.value}</span>
					<span class="mt-0.5 text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
						{m.label}
					</span>
				</div>
			{/each}

			<div class="flex flex-col items-center">
				<span class="flex items-center gap-1.5">
					<span class="text-xl font-semibold tabular-nums text-foreground">{formatCount(last7)}</span>
					{#if trendPct !== null}
						<span
							class="inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-semibold {trendUp
								? 'bg-success/10 text-success'
								: 'bg-foreground/5 text-muted-foreground'}"
						>
							{#if trendUp}
								<ArrowUpRight class="size-3" />
							{:else}
								<ArrowDownRight class="size-3" />
							{/if}
							{Math.abs(trendPct)}%
						</span>
					{/if}
				</span>
				<span class="mt-0.5 text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
					Last 7 days
				</span>
			</div>
		</div>
	{/if}
</section>
