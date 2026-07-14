<script lang="ts">
	import { prefersReducedMotion } from "$lib/motion-core";
	import { Reveal } from "@recast/ui/reveal";
	import { cn } from "@recast/ui/utils";
	import { Check } from "@lucide/svelte";

	// Step 2's auto-polish grid. A soft "Applied" check ticks through the cards
	// one at a time, reading as edits being applied automatically while you
	// record. Deliberately slower and gentler than Step 1's palette so the two
	// steps don't compete. Reduced motion shows the grid with nothing ticking.
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
	class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-4"
>
	{#each features as feature, i (feature.title)}
		{@const Icon = feature.icon}
		<Reveal variant="morph" delay={i * 80} class="h-full">
			<div
				class={cn(
					"flex h-full flex-col gap-3 bg-background/50 p-6 backdrop-blur-md transition-colors duration-500",
					i === applied && "bg-primary/5",
				)}
			>
				<div class="flex items-center justify-between">
					<Icon class="size-5 text-primary" />
					<span
						class={cn(
							"inline-flex items-center gap-1 rounded-full bg-success/12 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-success ring-1 ring-inset ring-success/25 transition-opacity duration-500",
							i === applied ? "opacity-100" : "opacity-0",
						)}
					>
						<Check class="size-2.5" />
						Applied
					</span>
				</div>
				<div>
					<div class="text-sm font-semibold text-foreground">{feature.title}</div>
					<div class="mt-1.5 text-sm leading-relaxed text-muted-foreground">
						{feature.description}
					</div>
				</div>
			</div>
		</Reveal>
	{/each}
</div>
