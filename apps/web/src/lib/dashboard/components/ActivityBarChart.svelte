<script lang="ts">
import { BarChart } from "layerchart";
import * as Chart from "$components/ui/chart/index.js";

let {
	data,
}: {
	data: { label: string; views: number }[];
} = $props();

const max = $derived(Math.max(1, ...data.map((d) => d.views)));
const total = $derived(data.reduce((s, d) => s + d.views, 0));

// Thin the x-axis to a handful of evenly-spaced ticks so 14/30-day ranges
// don't collapse into an unreadable wall of labels.
const xTicks = $derived(data.length > 8 ? 6 : undefined);

const chartConfig = {
	views: { label: "Views", color: "var(--color-primary)" },
} satisfies Chart.ChartConfig;
</script>

<div class="surface p-5">
	<div class="flex items-end justify-between gap-4">
		<div>
			<div class="font-display text-heading-sm font-medium tabular-nums text-foreground">
				{total}
			</div>
			<div class="text-caption text-muted-foreground">Views in range</div>
		</div>
		<div class="text-right text-caption tabular-nums text-muted-foreground">Peak {max}</div>
	</div>

	<Chart.Container config={chartConfig} class="mt-5 aspect-auto h-32 w-full">
		<BarChart
			{data}
			x="label"
			y="views"
			axis="x"
			grid={false}
			rule={false}
			bandPadding={0.35}
			padding={{ top: 8, bottom: 22 }}
			highlight={{ area: { class: "fill-primary/10" } }}
			props={{
				xAxis: { ticks: xTicks },
				bars: { radius: 3, fillOpacity: 0.9 },
			}}
		>
			{#snippet tooltip()}
				<Chart.Tooltip formatter={viewsRow} />
			{/snippet}
		</BarChart>
	</Chart.Container>
</div>

{#snippet viewsRow({ value }: { value: unknown })}
	<span class="text-muted-foreground">Views</span>
	<span class="ml-auto font-medium tabular-nums text-foreground">{value}</span>
{/snippet}
