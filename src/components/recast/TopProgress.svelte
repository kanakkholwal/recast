<script lang="ts">
	import { cn } from "$lib/utils";

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
		<div class="recast-top-progress h-full w-1/3 bg-primary"></div>
	{/if}
</div>

<style>
	.recast-top-progress {
		animation: recast-top-progress 1.1s ease-in-out infinite;
	}
	@keyframes recast-top-progress {
		0% {
			transform: translateX(-100%);
		}
		60% {
			transform: translateX(220%);
		}
		100% {
			transform: translateX(220%);
		}
	}
</style>
