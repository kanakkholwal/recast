<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import * as Command from "../command";
import * as Popover from "../popover";

// A searchable single-select: a rich button trigger over a filtered, grouped list. Composes our Popover + Command so items stay the shared restyled row; the list body is supplied by the caller.
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
</script>

<Popover.Root {open} onOpenChange={(v) => (open = v)}>
	<Popover.Trigger>
		{#snippet child({ props })}
			{@render trigger({ props })}
		{/snippet}
	</Popover.Trigger>
	<Popover.Content {align} {sideOffset} class={cn("min-w-56 p-0", contentClass)}>
		<Command.Root>
			<Command.Input {placeholder} />
			<Command.List class="scrollbar-transparent max-h-72">
				<Command.Empty class="py-6 text-center text-[11px] text-muted-foreground">
					{emptyText}
				</Command.Empty>
				{@render children()}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
