<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	let {
		children,
		class: className = "",
		size = "default",
		as: Tag = "div",
	}: {
		children: Snippet;
		class?: string;
		size?: "default" | "narrow" | "wide" | "full";
		as?: "div" | "section" | "main" | "article" | "header" | "footer";
	} = $props();

	const sizes = {
		narrow: "max-w-3xl",
		default: "max-w-6xl",
		wide: "max-w-7xl",
		full: "max-w-none",
	};
</script>

<svelte:element
	this={Tag}
	class={cn("mx-auto w-full px-6 sm:px-8 lg:px-10", sizes[size], className)}
>
	{@render children()}
</svelte:element>
