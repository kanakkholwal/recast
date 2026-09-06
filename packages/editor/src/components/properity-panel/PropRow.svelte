<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

// Fixed label column + control cell, so numeric fields line up down the panel

interface Props {
	label: string;
	children: Snippet;
	/** Top-align the label for multi-row control stacks (a nudge pad, two fields). */
	alignTop?: boolean;
	class?: string;
}

let { label, children, alignTop = false, class: className }: Props = $props();
</script>

<div
	class={cn(
		"flex gap-2",
		alignTop ? "items-start" : "items-center",
		className,
	)}
>
	<span
		class={cn(
			"w-20 shrink-0 truncate text-[11px] text-muted-foreground",
			alignTop && "pt-1.5 leading-none",
		)}
	>
		{label}
	</span>
	<div class="flex min-w-0 flex-1 items-center gap-1.5">{@render children()}</div>
</div>
