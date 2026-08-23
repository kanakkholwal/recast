<script lang="ts">
import { type IconComponent, Minus, TrendingDown, TrendingUp } from "@recast/icons";

let {
	icon: Icon,
	label,
	value,
	delta = null,
	deltaLabel = "vs previous period",
	hint,
}: {
	icon: IconComponent;
	label: string;
	value: string;
	/** % change against the prior period. `null` hides the chip (no baseline). */
	delta?: number | null;
	deltaLabel?: string;
	/** One short line under the label, for a metric that needs a definition. */
	hint?: string;
} = $props();

// The arrow carries the direction, so the reading survives without colour.
const Trend = $derived(
	delta === null || delta === 0 ? Minus : delta > 0 ? TrendingUp : TrendingDown,
);
const tone = $derived(delta !== null && delta > 0 ? "text-success" : "text-muted-foreground");
</script>

<div class="surface flex items-start gap-3.5 p-4">
	<Icon class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
	<div class="min-w-0 flex-1">
		<div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
			<span class="text-subheading font-medium tabular-nums text-foreground">{value}</span>
			{#if delta !== null}
				<span
					class="inline-flex items-center gap-0.5 text-caption font-medium tabular-nums {tone}"
					title={deltaLabel}
				>
					<Trend class="size-3" aria-hidden="true" />
					{delta > 0 ? "+" : ""}{delta}%
					<span class="sr-only">{deltaLabel}</span>
				</span>
			{/if}
		</div>
		<div class="truncate text-caption text-muted-foreground">{label}</div>
		{#if hint}
			<div class="mt-1 text-caption text-muted-foreground">{hint}</div>
		{/if}
	</div>
</div>
