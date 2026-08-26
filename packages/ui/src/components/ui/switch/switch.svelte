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
		"peer group inline-flex h-5.5 w-9 shrink-0 cursor-pointer items-center rounded-full border border-transparent outline-none transition-colors duration-200",
		"focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
		"data-[state=checked]:bg-primary",
		// --border-control, not --border/50: the off-track fill is only 1.17:1 on
		// a card, so the boundary is what has to carry the 3:1 for the off state.
		"data-[state=unchecked]:bg-input data-[state=unchecked]:ring-1 data-[state=unchecked]:ring-inset data-[state=unchecked]:ring-border-control",
		"disabled:cursor-not-allowed disabled:opacity-50",
		className,
	)}
	{...restProps}
>
	<!-- iOS-style press feedback: the thumb stretches while held and keeps its
	     leading edge pinned, then springs back on release. -->
	<SwitchPrimitive.Thumb
		data-slot="switch-thumb"
		class={cn(
			"pointer-events-none block size-4.5 rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.25),0_2px_6px_rgb(0_0_0/0.15)]",
			"transition-[transform,width] duration-200 ease-[cubic-bezier(0.23,1,0.32,1)]",
			"data-[state=unchecked]:translate-x-0.5 data-[state=checked]:translate-x-3.5",
			"motion-safe:group-active:w-5.5 motion-safe:group-active:data-[state=checked]:translate-x-2.5",
		)}
	/>
</SwitchPrimitive.Root>
