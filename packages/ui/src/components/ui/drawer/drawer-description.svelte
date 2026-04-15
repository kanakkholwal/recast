<script lang="ts">
	import { Drawer as DrawerPrimitive } from "vaul-svelte";
	import { cn } from "@recast/ui/utils";

	let {
		class: className,
		...restProps
	}: DrawerPrimitive.DescriptionProps = $props();
</script>

<DrawerPrimitive.Description
	data-slot="drawer-description"
	class={cn("text-muted-foreground text-sm", className)}
	{...restProps}
/>
