<script lang="ts">
import Logo from "$lib/logo.svelte";
import type { Snippet } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

let {
	title,
	description,
	footer,
	children,
}: {
	title: string;
	description?: string;
	footer?: Snippet;
	children: Snippet;
} = $props();
</script>

<div class="w-full max-w-sm" in:fly={{ y: 16, duration: 600, easing: cubicOut }}>
	<div class="flex flex-col items-center text-center">
		<a
			href="/"
			class="group/logo flex items-center gap-2.5"
			aria-label="Recast home"
		>
			<span
				class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
			>
				<Logo size="22" color="transparent" fill="currentColor" />
			</span>
			<span class="font-display text-body-lg font-medium text-foreground">
				Recast
			</span>
		</a>
		<h1 class="mt-7 font-display text-heading-sm text-foreground">
			{title}
		</h1>
		{#if description}
			<p class="mt-2 text-pretty text-body-sm text-muted-foreground">
				{description}
			</p>
		{/if}
	</div>

	<div class="surface mt-8 p-6">
		{@render children()}
	</div>

	{#if footer}
		<div class="mt-6 text-center text-body-sm text-muted-foreground">
			{@render footer()}
		</div>
	{/if}
</div>
