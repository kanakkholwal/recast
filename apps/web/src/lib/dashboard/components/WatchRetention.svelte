<script lang="ts">
import { TrendingDown } from "@recast/icons";
import { AreaChart } from "layerchart";
import * as Chart from "$components/ui/chart/index.js";

// Share of plays reaching each decile: it shows WHERE viewers drop off, which an average watch percentage hides.
let {
	data,
}: {
	data: { pct: number; reached: number }[];
} = $props();

// Anchor at 100% when there are plays, so drop-off reads from a full baseline like every retention graph.
const hasViews = $derived(data.some((d) => d.reached > 0));
const curve = $derived(hasViews ? [{ pct: 0, reached: 100 }, ...data] : data);

// The 50% mark is a useful "did they get past the intro" reference.
const midpoint = $derived(data.find((d) => d.pct === 50)?.reached ?? 0);

const chartConfig = {
	reached: { label: "Viewers reached", color: "var(--color-primary)" },
} satisfies Chart.ChartConfig;
</script>

<div class="surface h-full p-5">
	<header class="flex items-center justify-between gap-4">
		<div class="flex items-center gap-2">
			<TrendingDown class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Watch retention</h2>
		</div>
		<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
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
	<span class="ml-auto font-medium tabular-nums text-foreground">{value}%</span>
{/snippet}
