<script lang="ts">
	import type { Snippet } from "svelte";
	import type { IconComponent } from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	// Shared empty/zero state: chip icon + title + description + optional action.
	// Replaces the bespoke dashed-border blocks repeated across the library,
	// archived tab, and the side rails.
	let {
		icon: Icon,
		title,
		description,
		bordered = true,
		children,
	}: {
		icon: IconComponent;
		title: string;
		description?: string;
		/** Dashed border + larger padding (library/archived). Off for inline rails. */
		bordered?: boolean;
		/** Optional call-to-action below the copy. */
		children?: Snippet;
	} = $props();
</script>

<div
	class={`flex flex-col items-center justify-center text-center ${
		bordered ? "rounded-xl border border-dashed border-border-low/70 py-16" : "py-10"
	}`}
	in:fly={{ y: 12, duration: 360, easing: cubicOut }}
>
	<span class="glass-chip grid size-12 place-items-center rounded-xl text-muted-foreground">
		<Icon class="size-5" />
	</span>
	<h3 class="mt-4 text-sm font-semibold text-foreground">{title}</h3>
	{#if description}
		<p class="mt-1 max-w-xs text-xs text-muted-foreground">{description}</p>
	{/if}
	{#if children}
		<div class="mt-5">
			{@render children()}
		</div>
	{/if}
</div>
