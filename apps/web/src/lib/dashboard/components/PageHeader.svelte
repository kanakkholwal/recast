<script lang="ts">
	import type { Snippet } from "svelte";
	import type { IconComponent } from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	// Canonical dashboard page header: glass-chip icon + title + subtitle, with
	// an optional actions slot on the right. Unifies the hand-rolled headers
	// across home / recasts / analytics so they animate and align identically.
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
			<span class="glass-chip grid size-11 shrink-0 place-items-center rounded-xl text-primary">
				<Icon class="size-5" />
			</span>
		{/if}
		<div class="min-w-0">
			<h1 class="truncate text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
			{#if subtitle}
				<p class="mt-0.5 text-sm text-muted-foreground">{subtitle}</p>
			{/if}
		</div>
	</div>
	{#if children}
		<div class="flex shrink-0 items-center gap-2">
			{@render children()}
		</div>
	{/if}
</header>
