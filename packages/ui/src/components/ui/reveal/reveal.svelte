<script lang="ts">
	import type { Snippet } from "svelte";
	import { cn } from "@recast/ui/utils";

	type Tag = "div" | "section" | "article" | "li" | "header" | "ol" | "ul";

	type Props = {
		children: Snippet;
		class?: string;
		delay?: number;
		threshold?: number;
		rootMargin?: string;
		once?: boolean;
		as?: Tag;
	};

	let {
		children,
		class: className,
		delay = 0,
		threshold = 0.15,
		rootMargin = "0px 0px -8% 0px",
		once = true,
		as: Tag = "div",
	}: Props = $props();

	let visible = $state(false);

	function reveal(node: HTMLElement) {
		if (typeof IntersectionObserver === "undefined") {
			visible = true;
			return {};
		}

		const observer = new IntersectionObserver(
			([entry]) => {
				if (entry.isIntersecting) {
					visible = true;
					if (once) observer.disconnect();
				} else if (!once) {
					visible = false;
				}
			},
			{ threshold, rootMargin },
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
		visible
			? "opacity-100 translate-y-0"
			: "opacity-0 translate-y-3 motion-reduce:translate-y-0 motion-reduce:opacity-100",
		className,
	)}
>
	{@render children()}
</svelte:element>
