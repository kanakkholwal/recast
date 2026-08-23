<script lang="ts">
import type { IconComponent } from "@recast/icons";
import type { Snippet } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

let {
	icon: Icon,
	title,
	subtitle,
	children,
}: {
	icon?: IconComponent;
	title: string;
	subtitle?: string;
	/** Right-aligned actions (buttons, badges). */
	children?: Snippet;
} = $props();
</script>

<header
	class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between"
	in:fly={{ y: 12, duration: 500, easing: cubicOut }}
>
	<div class="flex min-w-0 items-center gap-3">
		{#if Icon}
			<Icon class="size-6 shrink-0 text-muted-foreground" />
		{/if}
		<div class="min-w-0">
			<h1 class="truncate font-display font-semibold text-heading-sm text-foreground">{title}</h1>
			{#if subtitle}
				<p class="mt-1 text-body-sm text-muted-foreground">{subtitle}</p>
			{/if}
		</div>
	</div>
	{#if children}
		<div class="flex shrink-0 items-center gap-2">
			{@render children()}
		</div>
	{/if}
</header>
