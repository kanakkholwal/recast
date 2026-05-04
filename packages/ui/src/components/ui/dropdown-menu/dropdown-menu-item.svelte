<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import {
		dropdownMenuItemSizeVariants,
		getDropdownMenuSize,
		type DropdownMenuSize,
	} from "./context";

	let {
		ref = $bindable(null),
		class: className,
		inset,
		size,
		variant = "default",
		...restProps
	}: DropdownMenuPrimitive.ItemProps & {
		inset?: boolean;
		size?: DropdownMenuSize;
		variant?: "default" | "destructive";
	} = $props();

	// Inherit from <Content size="…"> unless overridden per-item.
	const resolvedSize = $derived(size ?? getDropdownMenuSize());
</script>

<DropdownMenuPrimitive.Item
	bind:ref
	data-slot="dropdown-menu-item"
	data-inset={inset}
	data-variant={variant}
	class={cn(
		"focus:bg-accent focus:text-accent-foreground data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 dark:data-[variant=destructive]:focus:bg-destructive/20 data-[variant=destructive]:focus:text-destructive data-[variant=destructive]:*:[svg]:text-destructive not-data-[variant=destructive]:focus:**:text-accent-foreground rounded-md data-inset:pl-7 group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-[inset]:pl-8 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		dropdownMenuItemSizeVariants({ size: resolvedSize }),
		className
	)}
	{...restProps}
/>
