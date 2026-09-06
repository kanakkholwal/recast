<script lang="ts">
import { cn } from "@recast/ui/utils";
import { Popover as PopoverPrimitive } from "bits-ui";
import type { Snippet } from "svelte";
import { expoOut } from "svelte/easing";
import { prefersReducedMotion } from "svelte/motion";
import * as Command from "../command";

// A searchable single-select: a rich button trigger over a filtered, grouped list. Uses bits-ui Popover for positioning/dismiss/focus, but drives the open with a height-morph so the panel unfolds from the trigger. Item rows come from the caller.
let {
	open = $bindable(false),
	placeholder = "Search…",
	emptyText = "No results",
	align = "start",
	sideOffset = 6,
	contentClass,
	trigger,
	children,
}: {
	open?: boolean;
	placeholder?: string;
	emptyText?: string;
	align?: "start" | "center" | "end";
	sideOffset?: number;
	contentClass?: string;
	trigger: Snippet<[{ props: Record<string, unknown> }]>;
	children: Snippet;
} = $props();

// Grow from the trigger to the panel's natural height with a blur-in; a weighted cubic settle stands in for the beui spring.
function morph(node: HTMLElement) {
	const h = node.scrollHeight;
	return {
		duration: prefersReducedMotion.current ? 0 : 300,
		easing: expoOut,
		css: (t: number, u: number) =>
			`height:${t * h}px; opacity:${Math.min(1, t * 2)}; filter:blur(${u * 3}px);`,
	};
}
</script>

<PopoverPrimitive.Root bind:open>
	<PopoverPrimitive.Trigger>
		{#snippet child({ props })}
			{@render trigger({ props })}
		{/snippet}
	</PopoverPrimitive.Trigger>
	<PopoverPrimitive.Portal>
		<PopoverPrimitive.Content {align} {sideOffset} forceMount>
			{#snippet child({ props, wrapperProps, open: isOpen })}
				<div {...wrapperProps}>
					{#if isOpen}
						<div
							{...props}
							transition:morph
							class={cn(
								"z-50 w-(--bits-popover-anchor-width) min-w-56 origin-(--bits-popover-content-transform-origin) overflow-hidden rounded-xl border border-border/60 bg-popover/95 text-popover-foreground shadow-craft-lg ring-1 ring-foreground/5 backdrop-blur-xl",
								contentClass,
							)}
						>
							<Command.Root>
								<Command.Input {placeholder} />
								<Command.List class="scrollbar-transparent max-h-72">
									<Command.Empty class="py-6 text-center text-[11px] text-muted-foreground">
										{emptyText}
									</Command.Empty>
									{@render children()}
								</Command.List>
							</Command.Root>
						</div>
					{/if}
				</div>
			{/snippet}
		</PopoverPrimitive.Content>
	</PopoverPrimitive.Portal>
</PopoverPrimitive.Root>
