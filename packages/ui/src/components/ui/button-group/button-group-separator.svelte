<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { ComponentProps } from "svelte";
	import { Separator } from "../separator";

	let {
		ref = $bindable(null),
		class: className,
		orientation = "vertical",
		...restProps
	}: ComponentProps<typeof Separator> = $props();
</script>

<Separator
	bind:ref
	data-slot="button-group-separator"
	{orientation}
	class={cn(
		"bg-input relative self-stretch data-[orientation=horizontal]:mx-px data-[orientation=horizontal]:w-auto data-[orientation=vertical]:my-px data-[orientation=vertical]:h-auto",
		className
	)}
	{...restProps}
/>
