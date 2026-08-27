<script lang="ts">
import { CRAFT_OVERLAY_ANIMATION, cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
import ContextMenuPortal from "./context-menu-portal.svelte";
import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
import type { ComponentProps } from "svelte";
import {
	contextMenuContentSizeVariants,
	setContextMenuSize,
	type ContextMenuSize,
} from "./context";

let {
	ref = $bindable(null),
	size = "default",
	portalProps,
	class: className,
	preventScroll = false,
	...restProps
}: ContextMenuPrimitive.ContentProps & {
	size?: ContextMenuSize;
	portalProps?: WithoutChildrenOrChild<ComponentProps<typeof ContextMenuPortal>>;
} = $props();

// Propagate size to descendant Item / CheckboxItem / RadioItem / SubTrigger.
$effect(() => {
	setContextMenuSize(size);
});
</script>

<ContextMenuPortal {...portalProps}>
	<ContextMenuPrimitive.Content
		bind:ref
		data-slot="context-menu-content"
		data-size={size}
		{preventScroll}
		class={cn(
			CRAFT_OVERLAY_ANIMATION,
			// Unfold from the corner nearest the pointer (macOS-menu feel)
			// instead of scaling from the centre.
			"origin-(--bits-floating-transform-origin)",
			"ring-foreground/10 text-popover-foreground rounded-lg shadow-md ring-1 z-50 overflow-x-hidden overflow-y-auto outline-none data-[state=closed]:overflow-hidden relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:bg-foreground/10 **:data-[slot$=-item]:data-highlighted:bg-foreground/10 **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:bg-foreground/10 **:data-[slot$=-trigger]:aria-expanded:bg-foreground/10! **:data-[variant=destructive]:focus:bg-foreground/10! **:data-[variant=destructive]:text-accent-foreground! **:data-[variant=destructive]:**:text-accent-foreground!",
			contextMenuContentSizeVariants({ size }),
			className
		)}
		{...restProps}
	/>
</ContextMenuPortal>
