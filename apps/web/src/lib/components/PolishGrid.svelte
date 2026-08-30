<script lang="ts">
import { Check } from "@recast/icons";
import { Reveal } from "@recast/ui/reveal";
import { cn } from "@recast/ui/utils";
import { prefersReducedMotion } from "$lib/motion-core";

// A soft Applied highlight ticks through the cards, slower than Step 1 so the two don't compete; reduced motion shows the grid still.
type Feature = {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	icon: any;
	title: string;
	description: string;
};
let { features }: { features: Feature[] } = $props();

const reduced = $derived(prefersReducedMotion());
let applied = $state(-1);

$effect(() => {
	if (reduced) {
		applied = -1;
		return;
	}
	const id = setInterval(() => {
		if (!document.hidden) applied = (applied + 1) % features.length;
	}, 2200);
	return () => clearInterval(id);
});
</script>

<div
	class="mt-12 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4"
>
	{#each features as feature, i (feature.title)}
		{@const Icon = feature.icon}
		{@const isApplied = i === applied}
		<Reveal variant="up" delay={i * 70} class="h-full">
			<div
				class={cn(
					// GPU-only props; the resting background sits at 90% of the page surface so a tile reads above the panel, even in dark mode.
					"flex h-full flex-col gap-4 rounded-2xl border bg-background/90 p-7 backdrop-blur-md transition-[transform,box-shadow,background-color,border-color] duration-500 ease-out",
					isApplied
						? "scale-[1.015] border-primary/30 bg-background"
						: "border-border-low shadow-craft-sm",
				)}
			>
				<div class="flex items-center justify-between">
					<Icon
						class={cn(
							"size-5 transition-colors duration-500",
							isApplied ? "text-primary" : "text-muted-foreground",
						)}
					/>
					<span
						class={cn(
							"inline-flex items-center gap-1 rounded-full bg-success/12 px-1.5 py-0.5 text-caption font-bold font-medium text-success ring-1 ring-inset ring-success/25 transition-[opacity,transform] duration-500 ease-out",
							isApplied
								? "translate-y-0 opacity-100"
								: "-translate-y-1 opacity-0",
						)}
					>
						<Check class="size-2.5" />
						Applied
					</span>
				</div>
				<div>
					<div class="text-body font-semibold tracking-tight text-foreground">{feature.title}</div>
					<div class="mt-2 text-body-sm leading-relaxed text-muted-foreground">
						{feature.description}
					</div>
				</div>
			</div>
		</Reveal>
	{/each}
</div>

<!--
  Live-cursor strip: the same number of dots as tiles, filled in to mirror the
  cycling "applied" index. Reads as a progress readout without text, pure
  affordance, ignored under reduced motion (the inner dots never advance).
-->
{#if !reduced}
	<div
		aria-hidden="true"
		class="mt-6 flex items-center justify-center gap-1.5"
	>
		{#each features as _, i}
			<span
				class={cn(
					"size-1.5 rounded-full transition-[transform,background-color] duration-500 ease-out",
					i === applied
						? "scale-125 bg-primary"
						: "bg-foreground/20",
				)}
			></span>
		{/each}
	</div>
{/if}
