<script lang="ts">
	import * as Chart from "$components/ui/chart/index.js";
	import { engagementHeatmap, type EngagementMoment } from "$lib/dashboard/activity";
	import { formatDuration } from "$lib/dashboard/format";
	import { Flame } from "@recast/icons";
	import { BarChart } from "layerchart";

	// "Which moments did viewers actually react to" — reactions + comments
	// bucketed across the video's runtime. The tallest bar is the moment people
	// loved; hovering a bar shows its timestamp + split.
	let {
		moments,
		durationSec,
	}: {
		moments: EngagementMoment[];
		durationSec: number;
	} = $props();

	const heat = $derived(engagementHeatmap(moments, durationSec, 24));
	const totalReactions = $derived(moments.filter((m) => m.kind === "reaction").length);
	const totalComments = $derived(moments.filter((m) => m.kind === "comment").length);

	// Comments stack on top of reactions within each time slice. Two hues, not two
	// opacities of one: opacity alone is not a reliable way to tell stacked bands
	// apart. Reactions keep the brand accent as the dominant signal, comments sit
	// on neutral ink.
	const series = [
		{ key: "reactions", label: "Reactions", value: "reactions", color: "var(--color-primary)", props: { fillOpacity: 0.85 } },
		{ key: "comments", label: "Comments", value: "comments", color: "var(--color-foreground)", props: { fillOpacity: 0.35 } },
	];

	const chartConfig = {
		reactions: { label: "Reactions", color: "var(--color-primary)" },
		comments: { label: "Comments", color: "var(--color-foreground)" },
	} satisfies Chart.ChartConfig;
</script>

<section class="glass-card rounded-xl p-5">
	<header class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Flame class="size-4 text-muted-foreground" />
			<h2 class="text-sm font-semibold text-foreground">Engagement by moment</h2>
		</div>
		{#if heat.peakSec !== null && heat.max > 0}
			<span class="font-mono text-[11px] text-muted-foreground">
				Peak · <span class="font-semibold text-foreground">{formatDuration(heat.peakSec)}</span>
			</span>
		{/if}
	</header>

	{#if heat.max === 0}
		<p class="mt-4 text-xs text-muted-foreground">
			No reactions or comments yet. They'll show up here, pinned to when they happened.
		</p>
	{:else}
		<Chart.Container config={chartConfig} class="mt-5 aspect-auto h-28 w-full">
			<BarChart
				data={heat.bins}
				x="startSec"
				{series}
				seriesLayout="stack"
				axis={false}
				grid={false}
				rule={false}
				bandPadding={0.15}
				padding={{ top: 8 }}
				props={{ bars: { radius: 2 } }}
			>
				{#snippet tooltip()}
					<Chart.Tooltip labelFormatter={(v: unknown) => formatDuration(v as number)} />
				{/snippet}
			</BarChart>
		</Chart.Container>

		<div class="mt-3 flex items-center justify-between text-[10px] font-medium text-muted-foreground">
			<span>0:00</span>
			<div class="flex items-center gap-3">
				<span class="flex items-center gap-1">
					<span class="size-2 rounded-[2px] bg-primary/80"></span>{totalReactions} reactions
				</span>
				<span class="flex items-center gap-1">
					<span class="size-2 rounded-[2px] bg-foreground/35"></span>{totalComments} comments
				</span>
			</div>
			<span>{formatDuration(durationSec)}</span>
		</div>
	{/if}
</section>
