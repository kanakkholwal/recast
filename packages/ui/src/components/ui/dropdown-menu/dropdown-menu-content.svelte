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
		...restProps
	}: DropdownMenuPrimitive.ContentProps & {
		size?: DropdownMenuSize;
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DropdownMenuPortal>>;
	} = $props();

	// Propagate size to descendant Item / CheckboxItem / RadioItem / SubTrigger.
	setDropdownMenuSize(size);
</script>

<DropdownMenuPortal {...portalProps}>
	<DropdownMenuPrimitive.Content
		bind:ref
		data-slot="dropdown-menu-content"
		data-size={size}
		{sideOffset}
		{align}
		class={cn(
			CRAFT_OVERLAY_ANIMATION,
			"ring-foreground/10 text-popover-foreground rounded-lg shadow-md ring-1 z-50 w-(--bits-dropdown-menu-anchor-width) overflow-x-hidden overflow-y-auto outline-none data-closed:overflow-hidden relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:bg-foreground/10 **:data-[slot$=-item]:data-highlighted:bg-foreground/10 **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:bg-foreground/10 **:data-[slot$=-trigger]:aria-expanded:bg-foreground/10! **:data-[variant=destructive]:focus:bg-foreground/10! **:data-[variant=destructive]:text-accent-foreground! **:data-[variant=destructive]:**:text-accent-foreground!",
			dropdownMenuContentSizeVariants({ size }),
			className
		)}
		{...restProps}
	/>
</DropdownMenuPortal>
