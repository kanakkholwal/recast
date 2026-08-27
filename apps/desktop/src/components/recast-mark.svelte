<script lang="ts">
// The Recast mark for icon slots: the three bars alone, in currentColor.
// The disc is dropped deliberately — a filled circle next to a row of outline
// icons reads as a different kind of object, and at 12–16px the bars inside it
// turn to mush. Without it the mark inherits hover/disabled colour like any
// other icon.
import Logo from "$components/logo.svelte";

let {
	class: className = "",
	size = 16,
	fill = "none",
	color = "currentColor",
}: { class?: string; size?: string | number; fill?: string; color?: string } = $props();
</script>

<Logo fill={fill} color={color} {size} class={className} aria-hidden="true" />
