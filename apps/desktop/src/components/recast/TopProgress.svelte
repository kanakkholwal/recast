<script lang="ts">
	import { cn } from "@recast/ui/utils";

	let { active = false, class: className }: { active?: boolean; class?: string } = $props();
</script>

<div
	class={cn(
		"pointer-events-none absolute inset-x-0 top-0 z-30 h-[2px] overflow-hidden",
		className
	)}
	aria-hidden={!active}
>
	{#if active}
		<div class="animate-recast-indeterminate h-full w-1/3 rounded-full bg-primary"></div>
	{/if}
</div>
