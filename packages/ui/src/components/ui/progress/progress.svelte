<script lang="ts">
import { cn } from "@recast/ui/utils";
import { Progress as ProgressPrimitive } from "bits-ui";

let {
	ref = $bindable(null),
	class: className,
	max = 100,
	value,
	...restProps
}: ProgressPrimitive.RootProps = $props();

// `null` means the work is real but uncountable (a phase that has not reported a
// total yet). bits-ui maps it to aria-valuenow="indeterminate"; the bar shows a
// travelling sliver so it still reads as running rather than stuck at zero.
const indeterminate = $derived(value == null);
const pct = $derived(
	indeterminate ? 0 : Math.min(100, Math.max(0, ((value ?? 0) / (max || 1)) * 100)),
);
</script>

<ProgressPrimitive.Root
	bind:ref
	{value}
	{max}
	data-slot="progress"
	class={cn("bg-primary/15 relative h-1.5 w-full overflow-hidden rounded-full", className)}
	{...restProps}
>
	{#if indeterminate}
		<div class="indeterminate bg-primary absolute inset-y-0 left-0 w-1/3 rounded-full"></div>
	{:else}
		<div
			class="bg-primary h-full rounded-full transition-[width] duration-200 ease-out"
			style="width: {pct}%"
		></div>
	{/if}
</ProgressPrimitive.Root>

<style>
	.indeterminate {
		animation: progress-slide 1.4s ease-in-out infinite;
	}

	@keyframes progress-slide {
		from {
			transform: translateX(-100%);
		}
		to {
			transform: translateX(300%);
		}
	}

	/* A frozen sliver parked at the left would read as "33% done". Fill the track
	   instead, so the bar says "running, amount unknown". */
	@media (prefers-reduced-motion: reduce) {
		.indeterminate {
			width: 100%;
			opacity: 0.5;
			animation: none;
		}
	}
</style>
