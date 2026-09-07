<script lang="ts">
import { MousePointer2 } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

// A decorative selected-object frame around a headline word. The chrome is aria-hidden and a sibling of the text, so a child's clip container can't clip the handles.
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
