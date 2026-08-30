<script lang="ts">
// The bars alone in currentColor: a filled disc reads as a different object beside outline icons and turns to mush at 16px.
import Logo from "$components/logo.svelte";

let {
	class: className = "",
	size = 16,
	fill = "none",
	color = "currentColor",
}: { class?: string; size?: string | number; fill?: string; color?: string } = $props();
</script>

<Logo fill={fill} color={color} {size} class={className} aria-hidden="true" />
