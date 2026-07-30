<script lang="ts">
// The Recast mark for icon slots: the three bars alone, in currentColor.
// The disc is dropped deliberately — a filled circle next to a row of outline
// icons reads as a different kind of object, and at 12–16px the bars inside it
// turn to mush. Without it the mark inherits hover/disabled colour like any
// other icon.
import Logo from "$components/logo.svelte";

let { class: klass = "", size = 16 }: { class?: string; size?: string | number } = $props();
</script>

<Logo fill="none" color="currentColor" {size} class={klass} aria-hidden="true" />
