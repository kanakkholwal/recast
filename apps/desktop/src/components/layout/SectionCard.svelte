<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

interface Props {
	label: string;
	description?: string;
	/** Optional leading glyph in the section eyebrow. */
	icon?: Snippet;
	/** Anchor id (used by the command palette to jump to a section). */
	id?: string;
	children: Snippet;
	class?: string;
}

let { label, description, icon, id, children, class: className }: Props = $props();
</script>

<section {id} class={cn("flex flex-col gap-3", className)}>
	<div class="px-1">
		<h2
			class="flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
		>
			{#if icon}{@render icon()}{/if}
			{label}
		</h2>
		{#if description}
			<p class="mt-0.5 text-[11px] leading-relaxed text-muted-foreground/80">
				{description}
			</p>
		{/if}
	</div>
	<div
		class="divide-y divide-border/40 overflow-hidden rounded-2xl border border-border/50 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
	>
		{@render children()}
	</div>
</section>
