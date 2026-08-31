<script lang="ts">
import { Check } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import { Command as CommandPrimitive } from "bits-ui";

let {
	ref = $bindable(null),
	class: className,
	children,
	...restProps
}: CommandPrimitive.ItemProps = $props();
</script>

<CommandPrimitive.Item
	bind:ref
	data-slot="command-item"
	class={cn(
		"group/command-item data-selected:bg-foreground/8 data-selected:text-foreground data-selected:*:[svg]:text-foreground relative flex cursor-default items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] outline-hidden transition-colors duration-100 select-none in-data-[slot=dialog-content]:rounded-lg! data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<Check class="cn-command-item-indicator ml-auto opacity-0 group-has-[[data-slot=command-shortcut]]/command-item:hidden group-data-[checked=true]/command-item:opacity-100" />
</CommandPrimitive.Item>
