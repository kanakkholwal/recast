<script lang="ts">
	import { cn } from "@recast/ui/utils";
	let { children, class: className = "" } = $props();
</script>

<div class={cn("container mx-auto px-6 md:px-12 xl:px-24 max-w-7xl", className)}>
	{@render children()}
</div>