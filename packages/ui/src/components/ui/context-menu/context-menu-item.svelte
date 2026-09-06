<script lang="ts">
import { cn } from "@recast/ui/utils";
import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
import { type ContextMenuSize, contextMenuItemSizeVariants, getContextMenuSize } from "./context";

let {
	ref = $bindable(null),
	class: className,
	inset,
	size,
	variant = "default",
	...restProps
}: ContextMenuPrimitive.ItemProps & {
	inset?: boolean;
	size?: ContextMenuSize;
	variant?: "default" | "destructive";
} = $props();

// Inherit from <Content size="…"> unless overridden per-item.
const resolvedSize = $derived(size ?? getContextMenuSize());
</script>

<ContextMenuPrimitive.Item
	bind:ref
	data-slot="context-menu-item"
	data-inset={inset}
	data-variant={variant}
	class={cn(
		"focus:bg-accent focus:text-accent-foreground data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 dark:data-[variant=destructive]:focus:bg-destructive/20 data-[variant=destructive]:focus:text-destructive data-[variant=destructive]:*:[svg]:text-destructive not-data-[variant=destructive]:focus:**:text-accent-foreground rounded-lg data-inset:pl-7 group/context-menu-item relative flex cursor-default items-center outline-hidden select-none transition-colors duration-100 data-disabled:pointer-events-none data-disabled:opacity-50 data-[inset]:pl-8 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		contextMenuItemSizeVariants({ size: resolvedSize }),
		className
	)}
	{...restProps}
/>
