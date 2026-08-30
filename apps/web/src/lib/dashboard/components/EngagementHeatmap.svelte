<script lang="ts">
import { Flame } from "@recast/icons";
import { BarChart } from "layerchart";
import * as Chart from "$components/ui/chart/index.js";
import { type EngagementMoment, engagementHeatmap } from "$lib/dashboard/activity";
import { formatDuration } from "$lib/dashboard/format";

// Reactions and comments bucketed across the runtime: the tallest bar is the moment people loved.
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

// Two hues, not two opacities of one: opacity alone can't reliably tell stacked bands apart.
const series = [
	{
		key: "reactions",
		label: "Reactions",
		value: "reactions",
		color: "var(--color-primary)",
		props: { fillOpacity: 0.85 },
	},
	{
		key: "comments",
		label: "Comments",
		value: "comments",
		color: "var(--color-foreground)",
		props: { fillOpacity: 0.35 },
	},
];

const chartConfig = {
	reactions: { label: "Reactions", color: "var(--color-primary)" },
	comments: { label: "Comments", color: "var(--color-foreground)" },
} satisfies Chart.ChartConfig;
</script>

<section class="surface p-5">
	<header class="flex items-center justify-between gap-4">
		<div class="flex items-center gap-2">
			<Flame class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Engagement by moment</h2>
		</div>
		{#if heat.peakSec !== null && heat.max > 0}
			<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
				Peak <span class="font-medium text-foreground">{formatDuration(heat.peakSec)}</span>
			</span>
		{/if}
	</header>

	{#if heat.max === 0}
		<p class="mt-4 text-body-sm text-muted-foreground">
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

		<div class="mt-3 flex items-center justify-between text-caption tabular-nums text-muted-foreground">
			<span>0:00</span>
			<div class="flex items-center gap-3">
				<span class="flex items-center gap-1">
					<span class="size-2 rounded-xs bg-primary"></span>{totalReactions} reactions
				</span>
				<span class="flex items-center gap-1">
					<span class="size-2 rounded-xs bg-foreground/40"></span>{totalComments} comments
				</span>
			</div>
			<span>{formatDuration(durationSec)}</span>
		</div>
	{/if}
</section>
