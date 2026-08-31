<script lang="ts">
import { Select as SelectPrimitive } from "bits-ui";
import { cn, type WithoutChild } from "@recast/ui/utils";
import { Selector } from "@recast/icons";

let {
	ref = $bindable(null),
	class: className,
	children,
	size = "default",
	...restProps
}: WithoutChild<SelectPrimitive.TriggerProps> & {
	size?: "sm" | "default";
} = $props();
</script>

<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	data-size={size}
	class={cn(
		"bg-muted/60 ring-1 ring-inset ring-border/40 hover:bg-muted data-placeholder:text-muted-foreground focus-visible:ring-ring/60 aria-invalid:ring-destructive/50 gap-1.5 rounded-md py-2 pr-2 pl-2.5 text-xs transition-[background-color,box-shadow] select-none data-[size=default]:h-8 data-[size=sm]:h-7 *:data-[slot=select-value]:flex *:data-[slot=select-value]:gap-1.5 [&_svg:not([class*='size-'])]:size-4 flex w-fit items-center justify-between whitespace-nowrap outline-none disabled:cursor-not-allowed disabled:opacity-50 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<Selector class="text-muted-foreground size-4 pointer-events-none" />
</SelectPrimitive.Trigger>
