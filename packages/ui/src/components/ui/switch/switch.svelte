<script lang="ts">
	import { Switch as SwitchPrimitive } from "bits-ui";
	import { cn, type WithoutChildrenOrChild } from "@recast/ui/utils";

	let {
		ref = $bindable(null),
		checked = $bindable(false),
		class: className,
		...restProps
	}: WithoutChildrenOrChild<SwitchPrimitive.RootProps> = $props();
</script>

<SwitchPrimitive.Root
	bind:ref
	bind:checked
	data-slot="switch"
	class={cn(
		"peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border border-transparent outline-none transition-colors",
		"focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
		"data-[state=checked]:bg-primary",
		"data-[state=unchecked]:bg-input data-[state=unchecked]:ring-1 data-[state=unchecked]:ring-inset data-[state=unchecked]:ring-border/50",
		"disabled:cursor-not-allowed disabled:opacity-50",
		className,
	)}
	{...restProps}
>
	<SwitchPrimitive.Thumb
		data-slot="switch-thumb"
		class={cn(
			"pointer-events-none block size-4 rounded-full bg-card shadow-sm ring-0 transition-transform",
			"data-[state=checked]:translate-x-4.5 data-[state=unchecked]:translate-x-0.5",
		)}
	/>
</SwitchPrimitive.Root>
