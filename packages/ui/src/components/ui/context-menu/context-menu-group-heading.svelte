<script lang="ts">
import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
import { cn } from "@recast/ui/utils";
import type { ComponentProps } from "svelte";

let {
	ref = $bindable(null),
	class: className,
	inset,
	...restProps
}: ComponentProps<typeof ContextMenuPrimitive.GroupHeading> & {
	inset?: boolean;
} = $props();
</script>

<ContextMenuPrimitive.GroupHeading
	bind:ref
	data-slot="context-menu-group-heading"
	data-inset={inset}
	class={cn("px-2 py-1.5 text-sm font-semibold data-[inset]:ps-8", className)}
	{...restProps}
/>
