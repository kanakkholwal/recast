<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import type { Snippet } from "svelte";

	let {
		children,
		class: className = "",
		delay = 0,
		threshold = 0.15,
		as: Tag = "div",
	}: {
		children: Snippet;
		class?: string;
		delay?: number;
		threshold?: number;
		as?: "div" | "section" | "article" | "li" | "header";
	} = $props();

	let visible = $state(false);

	function reveal(node: HTMLElement) {
		const observer = new IntersectionObserver(
			([entry]) => {
				if (entry.isIntersecting) {
					visible = true;
					observer.disconnect();
				}
			},
			{ threshold, rootMargin: "0px 0px -8% 0px" },
		);
		observer.observe(node);
		return {
			destroy: () => observer.disconnect(),
		};
	}
</script>

<svelte:element
	this={Tag}
	use:reveal
	style={`transition-delay: ${delay}ms;`}
	class={cn(
		"transition-[opacity,transform] duration-700 ease-[cubic-bezier(0.16,1,0.3,1)] motion-reduce:transition-none",
		visible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4 motion-reduce:translate-y-0 motion-reduce:opacity-100",
		className,
	)}
>
	{@render children()}
</svelte:element>
