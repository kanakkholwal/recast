<script lang="ts">
	import { cn } from "@recast/ui/utils";
	let { children, class: className = "", id = "" } = $props();
</script>

<section {id} class={cn("w-full relative group", className)}>
	{@render children()}
</section>