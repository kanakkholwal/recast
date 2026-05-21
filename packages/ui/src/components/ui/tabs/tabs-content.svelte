<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "@recast/ui/utils";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.ContentProps = $props();
</script>

<!--
	A subtle slide+fade plays whenever a panel becomes active. Driven by
	`data-state="active"` rather than Svelte's `transition:` so it survives
	bits-ui's mount/unmount cycle (svelte `out:` directives don't fire
	reliably inside `Tabs.Content` because the panel is conditionally
	rendered as a sibling, not a child of `{#if}`).

	tw-animate-css supplies `animate-in`, `fade-in-*`, `slide-in-from-*`,
	and the `duration-*` utility. Keep this short (160ms) — tabs are a
	cheap navigation, not a hero transition.
-->
<TabsPrimitive.Content
	bind:ref
	data-slot="tabs-content"
	class={cn(
		"text-sm flex-1 outline-none",
		"data-[state=active]:animate-in data-[state=active]:fade-in-0 data-[state=active]:slide-in-from-bottom-1 data-[state=active]:duration-200 data-[state=active]:ease-out",
		className,
	)}
	{...restProps}
/>
