<script lang="ts">
import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
import { Minus } from "@recast/icons";
import { Check } from "@recast/icons";
import { cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	ref = $bindable(null),
	checked = $bindable(false),
	indeterminate = $bindable(false),
	class: className,
	children: childrenProp,
	...restProps
}: WithoutChildrenOrChild<ContextMenuPrimitive.CheckboxItemProps> & {
	children?: Snippet;
} = $props();
</script>

<ContextMenuPrimitive.CheckboxItem
	bind:ref
	bind:checked
	bind:indeterminate
	data-slot="context-menu-checkbox-item"
	class={cn(
		"focus:bg-accent focus:text-accent-foreground focus:**:text-accent-foreground gap-1.5 rounded-md py-1 pr-8 pl-1.5 text-sm data-inset:pl-7 [&_svg:not([class*='size-'])]:size-4 relative flex cursor-default items-center outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{#snippet children({ checked, indeterminate })}
		<span
			class="absolute right-2 flex items-center justify-center pointer-events-none"
			data-slot="context-menu-checkbox-item-indicator"
		>
			{#if indeterminate}
				<Minus  />
			{:else if checked}
				<Check  />
			{/if}
		</span>
		{@render childrenProp?.()}
	{/snippet}
</ContextMenuPrimitive.CheckboxItem>
