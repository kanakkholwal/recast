<script lang="ts">
	import { Drawer as DrawerPrimitive } from "vaul-svelte";
	import { cn } from "@recast/ui/utils";

	let {
		class: className,
		...restProps
	}: DrawerPrimitive.TitleProps = $props();
</script>

<DrawerPrimitive.Title
	data-slot="drawer-title"
	class={cn("font-heading text-foreground text-base font-medium", className)}
	{...restProps}
/>
