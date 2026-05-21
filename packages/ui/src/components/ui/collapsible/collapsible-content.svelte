<script lang="ts">
	import { Collapsible as CollapsiblePrimitive } from "bits-ui";
	import { cn } from "@recast/ui/utils";
	import { cubicOut } from "svelte/easing";
	import { slide } from "svelte/transition";

	/**
	 * Collapsible content panel. Drives open/close with Svelte's native `slide`
	 * transition (real height animation, not a CSS keyframe) so the surrounding
	 * layout reflows smoothly as the content expands/collapses.
	 *
	 * Override `duration` / `easing` per call-site for hero panels; the defaults
	 * are tuned for dense list rows (240ms, cubicOut).
	 *
	 * `hiddenUntilFound` is set to false because it overrides `forceMount` in
	 * bits-ui, and we need forceMount so the `child` snippet keeps running and
	 * `{#if open}` controls Svelte's transition lifecycle.
	 */
	let {
		ref = $bindable(null),
		class: className,
		duration = 240,
		easing = cubicOut,
		children,
		...restProps
	}: CollapsiblePrimitive.ContentProps & {
		duration?: number;
		easing?: (t: number) => number;
	} = $props();
</script>

<CollapsiblePrimitive.Content
	bind:ref
	forceMount
	hiddenUntilFound={false}
	data-slot="collapsible-content"
	{...restProps}
>
	{#snippet child({ props, open })}
		{#if open}
			<div
				{...props}
				transition:slide={{ duration, easing, axis: "y" }}
				class={cn("overflow-hidden", className)}
			>
				{@render children?.()}
			</div>
		{/if}
	{/snippet}
</CollapsiblePrimitive.Content>
