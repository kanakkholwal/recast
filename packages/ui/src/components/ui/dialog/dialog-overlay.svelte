<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import { CRAFT_OVERLAY_BACKDROP_ANIMATION, cn } from "@recast/ui/utils";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: DialogPrimitive.OverlayProps = $props();
</script>

<DialogPrimitive.Overlay
	bind:ref
	data-slot="dialog-overlay"
	class={cn(CRAFT_OVERLAY_BACKDROP_ANIMATION, "bg-black/10 supports-backdrop-filter:backdrop-blur-xs fixed inset-0 isolate z-50", className)}
	{...restProps}
/>
