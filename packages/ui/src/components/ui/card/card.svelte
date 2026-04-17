<script lang="ts">
	import type { HTMLAttributes } from "svelte/elements";
	import { cn, type WithElementRef } from "@recast/ui/utils";

	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "default",
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLDivElement>> & { size?: "default" | "sm" } = $props();
</script>

<div
	bind:this={ref}
	data-slot="card"
	data-size={size}
	class={cn(
		"bg-card text-card-foreground border-border/40 shadow-sm transition-all duration-200",
		"rounded-3xl p-6 md:p-8",
		"group/card flex flex-col gap-4 overflow-hidden",
		className
	)}
	{...restProps}
>
	{@render children?.()}
</div>
