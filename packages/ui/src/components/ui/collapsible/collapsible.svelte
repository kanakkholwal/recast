<script lang="ts">
	import { Collapsible as CollapsiblePrimitive } from "bits-ui";
	import { cn } from "@recast/ui/utils";

	let {
		ref = $bindable(null),
		open = $bindable(false),
		class: className,
		...restProps
	}: CollapsiblePrimitive.RootProps = $props();
</script>

<CollapsiblePrimitive.Root
	bind:ref
	bind:open
	data-slot="collapsible"
	class={cn(className)}
	{...restProps}
/>
