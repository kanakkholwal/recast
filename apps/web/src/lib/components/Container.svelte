<script lang="ts">
	import { cn } from "@recast/ui/utils";
	let { children, class: className = "" } = $props();
</script>

<div class={cn("container mx-auto px-4 md:px-8 max-w-7xl", className)}>
	{@render children()}
</div>