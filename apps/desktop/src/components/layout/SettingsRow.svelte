<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

interface Props {
	label: string;
	/** Supporting copy under the label. Reads full-width; the control sits right. */
	description?: string;
	/** The control (Switch, Segmented, Button…) rendered on the trailing edge. */
	children: Snippet;
	/** Render the control on its own line below the copy (wide controls). */
	stacked?: boolean;
	class?: string;
}

let { label, description, children, stacked = false, class: className }: Props = $props();
</script>

<div
	class={cn(
		"flex gap-4 px-4 py-3.5",
		stacked
			? "flex-col"
			: "items-center justify-between",
		className,
	)}
>
	<div class="min-w-0 flex-1">
		<p class="text-[12.5px] font-medium text-foreground">{label}</p>
		{#if description}
			<p class="mt-0.5 text-[11.5px] leading-relaxed text-muted-foreground">
				{description}
			</p>
		{/if}
	</div>
	<div
		class={cn(
			"flex items-center gap-2",
			stacked ? "justify-start" : "shrink-0 justify-end",
		)}
	>
		{@render children()}
	</div>
</div>
