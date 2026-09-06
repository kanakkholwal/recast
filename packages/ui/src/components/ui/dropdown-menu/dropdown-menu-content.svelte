<script lang="ts">
import { CRAFT_OVERLAY_ANIMATION, cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
import DropdownMenuPortal from "./dropdown-menu-portal.svelte";
import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
import type { ComponentProps } from "svelte";
import {
	dropdownMenuContentSizeVariants,
	setDropdownMenuSize,
	type DropdownMenuSize,
} from "./context";

let {
	ref = $bindable(null),
	sideOffset = 4,
	align = "start",
	size = "default",
	portalProps,
	class: className,
	preventScroll = false,
	...restProps
}: DropdownMenuPrimitive.ContentProps & {
	size?: DropdownMenuSize;
	portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DropdownMenuPortal>>;
} = $props();

// Propagate size to descendant Item / CheckboxItem / RadioItem / SubTrigger.
$effect(() => {
	setDropdownMenuSize(size);
});
</script>

<DropdownMenuPortal {...portalProps}>
	<DropdownMenuPrimitive.Content
		bind:ref
		data-slot="dropdown-menu-content"
		data-size={size}
		{sideOffset}
		{align}
		{preventScroll}
		class={cn(
			CRAFT_OVERLAY_ANIMATION,
			// Unfold from the corner nearest the trigger, macOS-menu style, rather than scaling from the centre.
			"origin-(--bits-floating-transform-origin)",
			"ring-foreground/10 text-popover-foreground rounded-xl shadow-lg ring-1 z-50 w-(--bits-dropdown-menu-anchor-width) overflow-x-hidden overflow-y-auto outline-none data-[state=closed]:overflow-hidden relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:bg-foreground/8 **:data-[slot$=-item]:data-highlighted:bg-foreground/8 **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:bg-foreground/8 **:data-[slot$=-trigger]:aria-expanded:bg-foreground/8! **:data-[variant=destructive]:focus:bg-destructive/10! **:data-[variant=destructive]:text-destructive! **:data-[variant=destructive]:**:text-destructive!",
			dropdownMenuContentSizeVariants({ size }),
			className
		)}
		{...restProps}
	/>
</DropdownMenuPortal>
