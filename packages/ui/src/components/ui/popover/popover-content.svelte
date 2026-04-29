<script lang="ts">
	import { Popover as PopoverPrimitive } from "bits-ui";
	import { cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
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
			"data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-72 origin-(--transform-origin) rounded-lg border border-border/50 bg-popover p-4 text-popover-foreground shadow-craft-lg outline-none ring-1 ring-foreground/5",
			className,
		)}
		{...restProps}
	/>
</PopoverPrimitive.Portal>
