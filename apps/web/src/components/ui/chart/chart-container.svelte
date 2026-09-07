<script lang="ts">
import type { HTMLAttributes } from "svelte/elements";
import { cn, type WithElementRef } from "$lib/utils.js";
import ChartStyle from "./chart-style.svelte";
import { type ChartConfig, setChartContext } from "./chart-utils.js";

const uid = $props.id();

let {
	ref = $bindable(null),
	id = uid,
	class: className,
	children,
	config,
	...restProps
}: WithElementRef<HTMLAttributes<HTMLElement>> & {
	config: ChartConfig;
} = $props();

const chartId = $derived(`chart-${id || uid.replace(/:/g, "")}`);

setChartContext({
	get config() {
		return config;
	},
});
</script>

<div
	bind:this={ref}
	data-chart={chartId}
	data-slot="chart"
	class={cn(
		"flex aspect-video justify-center overflow-visible text-xs",
		// Overrides: stroke around dots and marks on hover.
		"[&_.lc-highlight-point]:stroke-transparent",
		// override the default stroke color of lines
		"[&_.lc-line]:stroke-border/50",

		// by default, layerchart shows a line intersecting the point when hovering, this hides that
		"[&_.lc-highlight-line]:stroke-0",

		// Hovering a point on a stacked chart drops the other series' opacity by default; this overrides that.
		"[&_.lc-area-path]:opacity-100 [&_.lc-highlight-line]:opacity-100 [&_.lc-highlight-point]:opacity-100 [&_.lc-spline-path]:opacity-100 [&_.lc-text]:text-xs [&_.lc-text-svg]:overflow-visible",

		// Removing the stroke drops the tick lines; the alternative is disabling `tickMarks` on every chart's axes.
		"[&_.lc-axis-tick]:stroke-0",

		// The rule duplicates the grid line and renders after the marks, so it overlaps them.
		"[&_.lc-rule-x-line:not(.lc-grid-x-rule)]:stroke-0 [&_.lc-rule-y-line:not(.lc-grid-y-rule)]:stroke-0",
		"[&_.lc-grid-x-radial-line]:stroke-border [&_.lc-grid-x-radial-circle]:stroke-border",
		"[&_.lc-grid-y-radial-line]:stroke-border [&_.lc-grid-y-radial-circle]:stroke-border",

		// Legend adjustments
		"[&_.lc-legend-swatch-button]:items-center [&_.lc-legend-swatch-button]:gap-1.5",
		"[&_.lc-legend-swatch-group]:items-center [&_.lc-legend-swatch-group]:gap-4",
		"[&_.lc-legend-swatch]:size-2.5 [&_.lc-legend-swatch]:rounded-[2px]",

		// Labels
		"[&_.lc-labels-text:not([fill])]:fill-foreground [&_text]:stroke-transparent",

		// Tick labels on th x/y axes
		"[&_.lc-axis-tick-label]:fill-muted-foreground [&_.lc-axis-tick-label]:font-normal",
		"[&_.lc-tooltip-rects-g]:fill-transparent",
		"[&_.lc-layout-svg-g]:fill-transparent",
		"[&_.lc-root-container]:w-full",
		className
	)}
	{...restProps}
>
	<ChartStyle id={chartId} {config} />
	{@render children?.()}
</div>
