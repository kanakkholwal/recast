<script lang="ts">
import { ArrowUp } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import { prefersReducedMotion } from "$lib/motion-core";

/**
 * Back-to-top, shown only once the page is genuinely long-scrolled. Appearing
 * near the top would put a control over content for a trip the scrollbar
 * already makes trivial.
 */
let { showAfter = 0.5 }: { showAfter?: number } = $props();

const reduced = $derived(prefersReducedMotion());
let visible = $state(false);

function onScroll() {
	const scrollable = document.documentElement.scrollHeight - window.innerHeight;
	// A page barely taller than the viewport never earns the button.
	if (scrollable < 400) {
		visible = false;
		return;
	}
	visible = window.scrollY / scrollable >= showAfter;
}

function toTop() {
	window.scrollTo({ top: 0, behavior: reduced ? "auto" : "smooth" });
	// Send focus where the eye lands, or a keyboard user is left mid-page.
	document.querySelector<HTMLElement>("main, [role='main']")?.focus?.();
}
</script>

<svelte:window onscroll={onScroll} onresize={onScroll} />

<button
	type="button"
	onclick={toTop}
	aria-label="Back to top"
	tabindex={visible ? 0 : -1}
	aria-hidden={!visible}
	class={cn(
		"group fixed bottom-5 right-5 z-[60] grid size-11 cursor-pointer place-items-center rounded-full border border-border-low bg-card text-muted-foreground shadow-craft-floating outline-none",
		"transition-[opacity,transform] duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none",
		"hover:text-foreground focus-visible:ring-2 focus-visible:ring-primary",
		visible ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-3 opacity-0",
	)}
>
	<ArrowUp
		class="size-4 transition-transform duration-200 group-hover:-translate-y-0.5 motion-reduce:transition-none"
	/>
</button>
