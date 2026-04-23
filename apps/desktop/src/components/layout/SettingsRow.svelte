<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	interface Props {
		label: string;
		hint?: string;
		children: Snippet;
		class?: string;
	}

	let { label, hint, children, class: className }: Props = $props();
</script>

<div
	class={cn(
		"flex flex-col gap-3 px-5 py-4 sm:flex-row sm:items-center sm:gap-6",
		className,
	)}
>
	<div class="min-w-0 sm:w-44 sm:shrink-0">
		<p class="text-[12px] font-semibold text-foreground/85">{label}</p>
		{#if hint}
			<p class="mt-0.5 text-[11px] font-medium text-muted-foreground">
				{hint}
			</p>
		{/if}
	</div>
	<div class="flex min-w-0 flex-1 items-center justify-end gap-2">
		{@render children()}
	</div>
</div>