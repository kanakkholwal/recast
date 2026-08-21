<script lang="ts">
import type { IconComponent } from "@recast/icons";
import type { Snippet } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

// Heading block for every (auth) route. No card and no logo: the pane is the
// surface and the layout header owns the mark.
let {
	eyebrow,
	eyebrowIcon,
	title,
	description,
	footer,
	children,
}: {
	eyebrow?: string;
	eyebrowIcon?: IconComponent;
	title: string;
	description?: string;
	footer?: Snippet;
	children: Snippet;
} = $props();
</script>

<div in:fly={{ y: 12, duration: 480, easing: cubicOut }}>
	{#if eyebrow}
		{@const Icon = eyebrowIcon}
		<span class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground">
			{#if Icon}
				<Icon class="size-4" />
			{/if}
			{eyebrow}
		</span>
	{/if}

	<h1 class="mt-4 font-display text-balance text-heading text-foreground">
		{title}
	</h1>
	{#if description}
		<p class="mt-3 text-pretty text-body-sm text-muted-foreground">
			{description}
		</p>
	{/if}

	<div class="mt-8">
		{@render children()}
	</div>

	{#if footer}
		<div class="mt-6 text-body-sm text-muted-foreground">
			{@render footer()}
		</div>
	{/if}
</div>
