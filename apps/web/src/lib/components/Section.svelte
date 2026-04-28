<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	let {
		children,
		class: className = "",
		id,
		spacing = "default",
	}: {
		children: Snippet;
		class?: string;
		id?: string;
		spacing?: "tight" | "default" | "loose" | "none";
	} = $props();

	const spacings = {
		none: "",
		tight: "py-16 md:py-20",
		default: "py-24 md:py-32",
		loose: "py-32 md:py-44",
	};
</script>

<section {id} class={cn("relative w-full", spacings[spacing], className)}>
	{@render children()}
</section>
