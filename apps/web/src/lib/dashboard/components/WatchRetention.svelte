<script lang="ts">
	import * as Chart from "$components/ui/chart/index.js";
	import { TrendingDown } from "@lucide/svelte";
	import { AreaChart } from "layerchart";

	// Watch-retention survival curve: share of plays that reached each decile of
	// the video. Shows WHERE viewers drop off, which an average watch % hides.
	let {
		data,
	}: {
		data: { pct: number; reached: number }[];
	} = $props();

	// Retention is full at the very start of the video, so anchor the curve at
	// (0%, 100%) when there are plays — the drop-off then reads from a full
	// baseline, the way Instagram/YouTube retention graphs do.
	const hasViews = $derived(data.some((d) => d.reached > 0));
	const curve = $derived(hasViews ? [{ pct: 0, reached: 100 }, ...data] : data);

	// The 50% mark is a useful "did they get past the intro" reference.
	const midpoint = $derived(data.find((d) => d.pct === 50)?.reached ?? 0);

	const chartConfig = {
		reached: { label: "Viewers reached", color: "var(--color-primary)" },
	} satisfies Chart.ChartConfig;
</script>

<div class="glass-card rounded-xl p-5">
	<header class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<TrendingDown class="size-4 text-primary" />
			<h2 class="text-sm font-semibold text-foreground">Watch retention</h2>
		</div>
		<span class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			{midpoint}% reach halfway
		</span>
	</header>

	<Chart.Container config={chartConfig} class="mt-5 aspect-auto h-32 w-full">
		<AreaChart
			data={curve}
			x="pct"
			xDomain={[0, 100]}
			yDomain={[0, 100]}
			y="reached"
			axis="x"
			grid={false}
			rule={false}
			padding={{ top: 8, bottom: 22 }}
			props={{
				xAxis: { format: (v: number) => `${v}%`, ticks: [0, 25, 50, 75, 100] },
				area: { fillOpacity: 0.18, line: { class: "stroke-primary stroke-[1.5]" } },
				highlight: { points: { class: "fill-primary stroke-background" } },
			}}
		>
			{#snippet tooltip()}
				<Chart.Tooltip
					labelFormatter={(v: unknown) => `${v}% into video`}
					formatter={reachedRow}
				/>
			{/snippet}
		</AreaChart>
	</Chart.Container>
</div>

{#snippet reachedRow({ value }: { value: unknown })}
	<span class="text-muted-foreground">Reached</span>
	<span class="ml-auto font-mono font-medium tabular-nums text-foreground">{value}%</span>
{/snippet}
