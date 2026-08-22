<script lang="ts">
import SearchTrigger from "$lib/dashboard/components/SearchTrigger.svelte";
import { formatCount } from "$lib/dashboard/format";
import { ArrowDownRight, ArrowUpRight, BarChart3, Upload } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

let {
	firstName,
	totalViews,
	last7,
	trendPct,
	viewers,
	completion,
	avgWatch,
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
	onNew: () => void;
} = $props();

const trendUp = $derived((trendPct ?? 0) >= 0);

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
	<div class="flex flex-col items-center gap-2">
		<h1 class="font-display font-semibold text-balance text-heading text-foreground sm:text-heading-lg">
			Welcome back, {firstName}.
		</h1>
		<p class="max-w-md text-body-sm text-muted-foreground">
			{subtitle}
		</p>
	</div>

	<div class="w-full max-w-xl" in:fly={{ y: 12, duration: 500, delay: 60, easing: cubicOut }}>
		<SearchTrigger variant="hero" />
	</div>

	<!-- Actions -->
	<div class="flex items-center gap-2" in:fly={{ y: 12, duration: 500, delay: 120, easing: cubicOut }}>
		<Button variant="outline" size="sm" href="/dashboard/analytics" class="gap-2">
			<BarChart3 class="size-3.5" />
			Analytics
		</Button>
		<Button size="sm" variant="dark" class="gap-2" onclick={onNew}>
			<Upload class="size-3.5" />
			Upload
		</Button>
	</div>

	<!-- Demoted performance strip -->
	{#if noViews}
		<p class="max-w-md text-xs text-muted-foreground">
			No views yet. Share a recast to start tracking performance and engagement.
		</p>
	{:else}
		<div class="flex w-full flex-wrap items-start justify-center gap-x-10 gap-y-5 border-y border-border-low py-5">
			{#each metrics as m (m.label)}
				<div class="flex flex-col items-center">
					<span class="font-display text-subheading font-medium tabular-nums text-foreground">
						{m.value}
					</span>
					<span class="mt-1 text-caption text-muted-foreground">{m.label}</span>
				</div>
			{/each}

			<div class="flex flex-col items-center">
				<span class="flex items-center gap-1.5">
					<span class="font-display text-subheading font-medium tabular-nums text-foreground">
						{formatCount(last7)}
					</span>
					{#if trendPct !== null}
						<span
							class="inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-caption font-medium {trendUp
								? 'bg-success/12 text-success'
								: 'bg-paper text-muted-foreground'}"
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
				<span class="mt-1 text-caption text-muted-foreground">Last 7 days</span>
			</div>
		</div>
	{/if}
</section>
