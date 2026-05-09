<script lang="ts">
	import { Popover as PopoverPrimitive } from "bits-ui";
	import { CRAFT_OVERLAY_ANIMATION, cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
	import type { ComponentProps } from "svelte";

	type PortalProps = WithoutChildrenOrChild<ComponentProps<typeof PopoverPrimitive.Portal>>;

	let {
		ref = $bindable(null),
		class: className,
		align = "center",
		sideOffset = 6,
		portalProps,
		...restProps
	}: PopoverPrimitive.ContentProps & {
		portalProps?: PortalProps;
	} = $props();
</script>

<PopoverPrimitive.Portal {...portalProps}>
	<PopoverPrimitive.Content
		bind:ref
		data-slot="popover-content"
		{align}
		{sideOffset}
		class={cn(
			CRAFT_OVERLAY_ANIMATION,
			"z-50 w-72 origin-(--transform-origin) rounded-lg border border-border/50 bg-popover p-4 text-popover-foreground shadow-craft-lg outline-none ring-1 ring-foreground/5",
			className,
		)}
		{...restProps}
	/>
</PopoverPrimitive.Portal>
