<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";
	import type { IconComponent } from "@recast/icons";

	let {
		icon: Icon,
		title,
		description,
		accent = false,
		badge,
		children,
	}: {
		icon: IconComponent;
		title: string;
		description?: string;
		/** Rings the card. Marks the plan the workspace is on, or is being sold. */
		accent?: boolean;
		badge?: Snippet;
		children: Snippet;
	} = $props();
</script>

<section class={cn("glass-card rounded-xl p-6", accent && "ring-1 ring-foreground/12")}>
	<div class="flex items-center gap-3">
		<span class="glass-chip grid size-9 place-items-center rounded-lg text-muted-foreground">
			<Icon class="size-4" />
		</span>
		<div class="flex-1">
			<h2 class="text-sm font-semibold text-foreground">{title}</h2>
			{#if description}
				<p class="text-xs text-muted-foreground">{description}</p>
			{/if}
		</div>
		{#if badge}
			{@render badge()}
		{/if}
	</div>
	<div class="mt-5">
		{@render children()}
	</div>
</section>
