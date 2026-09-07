<script lang="ts" module>
import type { Snippet } from "svelte";

export interface PropRowProps {
	label: string;
	children: Snippet;
	/** Top-align the label for multi-row control stacks (a nudge pad, two fields). */
	alignTop?: boolean;
	class?: string;
}
</script>

<script lang="ts">
	import { cn } from "@recast/ui/utils";

	let { label, children, alignTop = false, class: className }: PropRowProps = $props();
</script>

<div class={cn("flex gap-2", alignTop ? "items-start" : "items-center", className)}>
	<span
		class={cn(
			"text-muted-foreground w-20 shrink-0 truncate text-[11px]",
			alignTop && "pt-1.5 leading-none",
		)}
	>
		{label}
	</span>
	<div class="flex min-w-0 flex-1 items-center gap-1.5">{@render children()}</div>
</div>
