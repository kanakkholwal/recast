<script lang="ts">
import { MousePointer2 } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

// Wraps a headline word in a decorative "selected object" frame: primary
// bounding box, corner resize handles, and a cursor arrow. It says the thing
// you ship is editable, which is the whole product promise. The chrome is
// aria-hidden and sits as a sibling of the real text (never a parent), so a
// child with its own clip container (TextLoop) can't clip the handles, and
// the absolute box tracks the child's width as it animates.
let { children, class: className }: { children: Snippet; class?: string } = $props();

// Handles pinned just outside each corner of the box.
const corners = ["-left-1 -top-1", "-right-1 -top-1", "-left-1 -bottom-1", "-right-1 -bottom-1"];
</script>

<span class={cn("relative inline-grid", className)}>
	<span
		aria-hidden="true"
		class="pointer-events-none absolute -inset-x-1.5 -inset-y-1 rounded-[3px] border border-primary bg-primary/6"
	>
		{#each corners as pos (pos)}
			<span
				class="absolute {pos} size-1.5 rounded-[1px] border border-primary bg-background"
			></span>
		{/each}
		<span class="absolute -bottom-4 -right-4 text-primary">
			<MousePointer2 class="size-3.5 fill-primary/20" />
		</span>
	</span>
	{@render children()}
</span>
