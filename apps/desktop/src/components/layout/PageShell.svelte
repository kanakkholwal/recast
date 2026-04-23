<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	interface Props {
		title: string;
		subtitle?: string;
		toolbar?: Snippet;
		children: Snippet;
		class?: string;
		contentClass?: string;
	}

	let {
		title,
		subtitle,
		toolbar,
		children,
		class: className,
		contentClass,
	}: Props = $props();
</script>

<div
	class={cn(
		"relative flex h-full flex-col bg-background font-sans select-none",
		className,
	)}
>
	<header
		class="flex shrink-0 items-center justify-between gap-4 border-b border-border/30 px-8 pt-7 pb-5"
	>
		<div class="min-w-0 space-y-1">
			<h1 class="truncate text-[18px] font-semibold tracking-tight text-foreground">
				{title}
			</h1>
			{#if subtitle}
				<p class="truncate text-[12px] font-medium text-muted-foreground">
					{subtitle}
				</p>
			{/if}
		</div>
		{#if toolbar}
			<div class="flex shrink-0 items-center gap-1.5">
				{@render toolbar()}
			</div>
		{/if}
	</header>

	<div class={cn("flex-1 overflow-y-auto scrollbar-transparent", contentClass)}>
		{@render children()}
	</div>
</div>
