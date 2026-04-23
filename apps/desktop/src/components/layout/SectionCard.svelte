<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	interface Props {
		label: string;
		description?: string;
		children: Snippet;
		class?: string;
	}

	let { label, description, children, class: className }: Props = $props();
</script>

<section
	class={cn(
		"rounded-2xl bg-card/40 ring-1 ring-inset ring-border/50 shadow-(--shadow-craft-inset)",
		className,
	)}
>
	<header class="border-b border-border/30 px-5 py-3">
		<h2
			class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground"
		>
			{label}
		</h2>
		{#if description}
			<p class="mt-1 text-[11px] font-medium text-muted-foreground/70">
				{description}
			</p>
		{/if}
	</header>
	<div class="divide-y divide-border/30">
		{@render children()}
	</div>
</section>
