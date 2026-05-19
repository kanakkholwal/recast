<script lang="ts">
	import type { Snippet } from "svelte";
	import { cn } from "@recast/ui/utils";

	type Tag = "div" | "section" | "article" | "li" | "header" | "ol" | "ul";

	/**
	 * Scroll-triggered reveal. Svelte's `transition:` directives only fire on
	 * mount/destroy, so scroll-in motion is driven by an IntersectionObserver
	 * toggling Tailwind utility classes through the shared `CRAFT_EASE` curve.
	 *
	 * Variants:
	 * - `up` / `down` / `left` / `right` — directional slide + fade
	 * - `blur` — focus-pull blur + fade
	 * - `scale` — gentle scale + fade
	 * - `morph` — combined blur + scale + slide (the "settle into place" feel)
	 */
	type Variant = "up" | "down" | "left" | "right" | "blur" | "scale" | "morph";

	type Props = {
		children: Snippet;
		class?: string;
		delay?: number;
		threshold?: number;
		rootMargin?: string;
		once?: boolean;
		as?: Tag;
		variant?: Variant;
	};

	let {
		children,
		class: className,
		delay = 0,
		threshold = 0.15,
		rootMargin = "0px 0px -8% 0px",
		once = true,
		as: Tag = "div",
		variant = "up",
	}: Props = $props();

	let visible = $state(false);

	const hidden: Record<Variant, string> = {
		up: "opacity-0 translate-y-3",
		down: "opacity-0 -translate-y-3",
		left: "opacity-0 translate-x-5",
		right: "opacity-0 -translate-x-5",
		blur: "opacity-0 blur-md",
		scale: "opacity-0 scale-[0.97]",
		morph: "opacity-0 translate-y-3 scale-[0.98] blur-[6px]",
	};

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
		"transition-[opacity,transform,filter] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] will-change-[opacity,transform,filter] motion-reduce:transition-none",
		visible
			? "opacity-100 translate-x-0 translate-y-0 scale-100 blur-0"
			: cn(hidden[variant], "motion-reduce:translate-x-0 motion-reduce:translate-y-0 motion-reduce:scale-100 motion-reduce:blur-0 motion-reduce:opacity-100"),
		className,
	)}
>
	{@render children()}
</svelte:element>
