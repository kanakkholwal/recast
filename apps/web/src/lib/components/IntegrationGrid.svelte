<script lang="ts" module>
import type { IconComponent } from "@recast/icons";

export type Integration = { icon: IconComponent; label: string; live?: boolean };
</script>

<script lang="ts">
import { Reveal } from "@recast/ui/reveal";
import { cn } from "@recast/ui/utils";

// Scattered field of destinations. Empty cells are load-bearing: a full grid
// reads as a finished list, a sparse one reads as a set still filling up.
let {
	items,
	columns = 5,
	rows = 4,
}: { items: Integration[]; columns?: number; rows?: number } = $props();

// Fixed slot indices so the scatter is stable between renders, not random.
const SLOTS = [1, 3, 5, 7, 8, 11, 13, 15, 16, 18, 2, 6, 10, 14, 17];

const cells = $derived.by(() => {
	const total = columns * rows;
	const filled = new Map<number, Integration>();
	items.forEach((item, i) => filled.set(SLOTS[i % SLOTS.length] % total, item));
	return Array.from({ length: total }, (_, i) => filled.get(i) ?? null);
});
</script>

<div
	class="grid gap-px border-y border-border-low bg-border-low"
	style={`grid-template-columns: repeat(${columns}, minmax(0, 1fr))`}
>
	{#each cells as cell, i (i)}
		{#if cell}
			{@const Icon = cell.icon}
			<Reveal
				variant="up"
				delay={i * 30}
				class="group/cell relative flex aspect-square items-center justify-center bg-background"
			>
				<Icon
					class={cn(
						"size-7 transition-colors duration-300 motion-reduce:transition-none",
						cell.live ? "text-foreground" : "text-border-strong group-hover/cell:text-foreground",
					)}
				/>
				<span class="sr-only">{cell.label}{cell.live ? ", available today" : ", planned"}</span>
				{#if cell.live}
					<span
						aria-hidden="true"
						class="absolute right-2.5 top-2.5 size-1.5 rounded-full bg-tag-green"
					></span>
				{/if}
			</Reveal>
		{:else}
			<div aria-hidden="true" class="aspect-square bg-background"></div>
		{/if}
	{/each}
</div>
