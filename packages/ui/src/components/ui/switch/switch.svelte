<script lang="ts">
import { cn } from "@recast/ui/utils";
import { prefersReducedMotion, Spring } from "svelte/motion";

interface Props {
	ref?: HTMLButtonElement | null;
	checked?: boolean;
	onCheckedChange?: (checked: boolean) => void;
	disabled?: boolean;
	class?: string;
}

let {
	ref = $bindable(null),
	checked = $bindable(false),
	onCheckedChange,
	disabled = false,
	class: className,
	...restProps
}: Props & Record<string, unknown> = $props();

// Off→on as 0→1. Snappy travel with a light settle: enough stiffness to feel instant, damping high enough to avoid visible wobble.
const pos = new Spring(checked ? 1 : 0, { stiffness: 0.2, damping: 0.78 });
let pressed = $state(false);
let thumbEl = $state<HTMLSpanElement | null>(null);

$effect(() => {
	pos.set(checked ? 1 : 0, { instant: prefersReducedMotion.current });
});

const squish = $derived(!disabled && pressed && !prefersReducedMotion.current);
const TRAVEL = 20; // 48px track − 8px padding − 20px thumb.

function toggle() {
	if (disabled) return;
	const next = !checked;
	checked = next;
	onCheckedChange?.(next);
}

// Disabled + pressed: a short shake says "not here" without changing state. Identity transform while disabled, so it can own the thumb transform for its run.
$effect(() => {
	if (!thumbEl || !disabled || !pressed || prefersReducedMotion.current) return;
	const anim = thumbEl.animate(
		[
			{ transform: "translateX(0)" },
			{ transform: "translateX(-2px)" },
			{ transform: "translateX(2px)" },
			{ transform: "translateX(-1px)" },
			{ transform: "translateX(0)" },
		],
		{ delay: 200, duration: 600, easing: "cubic-bezier(0.36,0.07,0.19,0.97)" },
	);
	return () => anim.cancel();
});
</script>

<button
	bind:this={ref}
	type="button"
	role="switch"
	aria-checked={checked}
	{disabled}
	data-slot="switch"
	data-state={checked ? "checked" : "unchecked"}
	onclick={toggle}
	onpointerdown={() => (pressed = true)}
	onpointerup={() => (pressed = false)}
	onpointerleave={() => (pressed = false)}
	class={cn(
		"group relative inline-flex h-7 w-12 shrink-0 cursor-pointer items-center rounded-full px-1 outline-none",
		"transition-colors duration-150 ease-out",
		"focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2 focus-visible:ring-offset-background",
		"disabled:cursor-not-allowed disabled:opacity-60",
		checked ? "bg-primary" : "bg-muted-foreground/55",
		className,
	)}
	{...restProps}
>
	<span class="pointer-events-none block" style="transform: translateX({pos.current * TRAVEL}px);">
		<!-- Squish scales toward the destination edge, so the thumb leans into its travel like weight shifting. -->
		<span
			bind:this={thumbEl}
			class="block size-5 rounded-full bg-background shadow-craft-md transition-transform duration-100 ease-out"
			style="transform: scaleX({squish ? 1.14 : 1}) scaleY({squish ? 0.9 : 1}); transform-origin: {checked
				? 'right'
				: 'left'} center;"
		></span>
	</span>
</button>
