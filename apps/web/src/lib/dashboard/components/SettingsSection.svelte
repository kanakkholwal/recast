<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Component, Snippet } from "svelte";

	let {
		icon: Icon,
		title,
		description,
		tone = "primary",
		accent = false,
		badge,
		children,
	}: {
		icon: Component<{ class?: string }>;
		title: string;
		description?: string;
		tone?: "primary" | "muted";
		/** Adds a primary ring — used to mark the priority integration. */
		accent?: boolean;
		badge?: Snippet;
		children: Snippet;
	} = $props();
</script>

<section class={cn("glass-card rounded-xl p-6", accent && "ring-1 ring-primary/15")}>
	<div class="flex items-center gap-3">
		<span
			class={cn(
				"glass-chip grid size-9 place-items-center rounded-lg",
				tone === "primary" ? "text-primary" : "text-muted-foreground",
			)}
		>
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
