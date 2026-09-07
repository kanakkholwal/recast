<script lang="ts">
import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
import { CRAFT_OVERLAY_ANIMATION, cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
import DropdownMenuPortal from "./dropdown-menu-portal.svelte";
import type { ComponentProps } from "svelte";

let {
	ref = $bindable(null),
	class: className,
	portalProps,
	...restProps
}: DropdownMenuPrimitive.SubContentProps & {
	portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DropdownMenuPortal>>;
} = $props();
</script>

<!-- Portal the submenu to <body>, same as the root Content. Without this the
     SubContent renders *inside* the scrollable Content (which carries
     `overflow-x-hidden overflow-y-auto` for long menus), so a submenu opening
     to the side gets clipped. Floating-ui anchors it off the SubTrigger
     regardless of DOM location, so portaling is safe and fixes the clipping. -->
<DropdownMenuPortal {...portalProps}>
	<DropdownMenuPrimitive.SubContent
		bind:ref
		data-slot="dropdown-menu-sub-content"
		class={cn(CRAFT_OVERLAY_ANIMATION, "origin-(--bits-floating-transform-origin) ring-foreground/10 text-popover-foreground min-w-[96px] rounded-lg p-1 shadow-lg ring-1 w-auto relative bg-popover/70 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 **:data-[slot$=-item]:focus:bg-foreground/10 **:data-[slot$=-item]:data-highlighted:bg-foreground/10 **:data-[slot$=-separator]:bg-foreground/5 **:data-[slot$=-trigger]:focus:bg-foreground/10 **:data-[slot$=-trigger]:aria-expanded:bg-foreground/10! **:data-[variant=destructive]:focus:bg-foreground/10! **:data-[variant=destructive]:text-accent-foreground! **:data-[variant=destructive]:**:text-accent-foreground!", className)}
		{...restProps}
	/>
</DropdownMenuPortal>
