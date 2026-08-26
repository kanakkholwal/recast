<script lang="ts" module>
import { tv, type VariantProps } from "tailwind-variants";
export const tabsListVariants = tv({
	base: "rounded-lg p-[3px] group-data-horizontal/tabs:h-9 data-[variant=line]:rounded-none group/tabs-list text-muted-foreground inline-flex w-fit items-center justify-center group-data-[orientation=vertical]/tabs:h-fit group-data-[orientation=vertical]/tabs:flex-col",
	variants: {
		variant: {
			default: "gap-2-list-variant-default bg-muted",
			line: "rounded-lg p-[3px] group-data-horizontal/tabs:h-9 data-[variant=line]:rounded-none-variant-line gap-1 bg-transparent",
			soft: 'bg-muted/60 [&_[data-slot="tabs-trigger"][data-state=active]_svg]:text-primary [&_[data-slot="tabs-trigger"][data-state=active]]:text-foreground [&_[data-slot="tabs-trigger"]]:text-muted-foreground [&_[data-slot="tabs-trigger"]:hover]:text-foreground [&_[data-slot="tabs-trigger"]]:shadow-transparent',
			// Bare nav rail: no list chrome; the active trigger rides a dark
			// bg-foreground pill (same language as the app's notched top nav).
			pill: 'bg-transparent p-0 [&_[data-slot="tabs-trigger"]]:text-muted-foreground [&_[data-slot="tabs-trigger"]:hover:not([data-state=active])]:text-foreground [&_[data-slot="tabs-trigger"][data-state=active]]:text-background [&_[data-slot="tabs-trigger"][data-state=active]]:bg-foreground [&_[data-slot="tabs-trigger"]]:shadow-transparent',
		},
	},
	defaultVariants: {
		variant: "default",
	},
});
export type TabsListVariant = VariantProps<typeof tabsListVariants>["variant"];
</script>

<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cubicOut } from "svelte/easing";
	import { Tween } from "svelte/motion";

	let {
		ref = $bindable(null),
		variant = "default",
		class: className,
		children,
		...restProps
	}: TabsPrimitive.ListProps & {
		variant?: TabsListVariant;
	} = $props();

	// Floating active indicator. Measures the currently-active trigger and
	// Tweens position/size to it, so switching tabs slides the fill from
	// one trigger to the next instead of snapping. Driven entirely by DOM
	// observation so it stays decoupled from bits-ui's value state.
	let indicatorVisible = $state(false);
	let isVertical = $state(false);
	let firstMeasure = true;

	// The indicator slides via Tween (rAF), which the CSS prefers-reduced-motion
	// override can't reach; honor the setting in JS so it snaps instead.
	let reduced = $state(false);
	$effect(() => {
		if (typeof window === "undefined") return;
		const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
		reduced = mq.matches;
		const on = (e: MediaQueryListEvent) => (reduced = e.matches);
		mq.addEventListener("change", on);
		return () => mq.removeEventListener("change", on);
	});

	const x = new Tween(0, { duration: 260, easing: cubicOut });
	const y = new Tween(0, { duration: 260, easing: cubicOut });
	const w = new Tween(0, { duration: 260, easing: cubicOut });
	const h = new Tween(0, { duration: 260, easing: cubicOut });

	function syncIndicator() {
		const el = ref as HTMLElement | null;
		if (!el) return;
		const active = el.querySelector<HTMLElement>(
			'[data-slot="tabs-trigger"][data-state="active"]',
		);
		if (!active) {
			indicatorVisible = false;
			return;
		}
		const listRect = el.getBoundingClientRect();
		const tRect = active.getBoundingClientRect();
		const nx = tRect.left - listRect.left;
		const ny = tRect.top - listRect.top;
		const nw = tRect.width;
		const nh = tRect.height;

		isVertical =
			el.dataset.orientation === "vertical" ||
			el.closest('[data-orientation="vertical"]') !== null;

		// Snap on first measure so the indicator doesn't grow from (0,0) —
		// fighting the dialog/page enter motion. Subsequent updates Tween.
		// Snap on the first measure (so it doesn't grow from 0,0 against page-enter
		// motion) and whenever the user asked for reduced motion; otherwise Tween.
		const duration = firstMeasure || reduced ? 0 : 260;
		x.set(nx, { duration });
		y.set(ny, { duration });
		w.set(nw, { duration });
		h.set(nh, { duration });
		firstMeasure = false;
		indicatorVisible = true;
	}

	$effect(() => {
		const el = ref as HTMLElement | null;
		if (!el) return;
		syncIndicator();
		const mo = new MutationObserver(() => syncIndicator());
		mo.observe(el, {
			subtree: true,
			attributes: true,
			attributeFilter: ["data-state"],
		});
		const ro = new ResizeObserver(() => syncIndicator());
		ro.observe(el);
		el
			.querySelectorAll('[data-slot="tabs-trigger"]')
			.forEach((t) => ro.observe(t));
		return () => {
			mo.disconnect();
			ro.disconnect();
		};
	});
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	data-variant={variant}
	class={cn(
		"relative",
		tabsListVariants({ variant }),
		// When the floating indicator is live, suppress each trigger's own
		// active background/shadow so the indicator owns the visual. Triggers
		// get z-10 so their label + icon sit above the indicator pill.
		indicatorVisible &&
			variant !== "line" && [
				"[&_[data-slot=tabs-trigger][data-state=active]]:!bg-transparent",
				"[&_[data-slot=tabs-trigger][data-state=active]]:!shadow-none",
				"[&_[data-slot=tabs-trigger]]:z-10",
			],
		indicatorVisible &&
			variant === "line" && [
				"[&_[data-slot=tabs-trigger][data-state=active]]:after:opacity-0",
				"[&_[data-slot=tabs-trigger]]:z-10",
			],
		className,
	)}
	{...restProps}
>
	{#if indicatorVisible && variant !== "line"}
		<span
			aria-hidden="true"
			class={cn(
				"pointer-events-none absolute left-0 top-0 z-0 rounded-md will-change-transform",
				variant === "soft" && "bg-card shadow-(--shadow-craft-inset)",
				variant === "default" && "bg-background shadow-sm",
				variant === "pill" && "rounded-lg bg-foreground shadow-craft-md",
			)}
			style="transform: translate({x.current}px, {y.current}px); width: {w.current}px; height: {h.current}px;"
		></span>
	{/if}
	{#if indicatorVisible && variant === "line"}
		<span
			aria-hidden="true"
			class="pointer-events-none absolute z-0 bg-foreground will-change-transform"
			style={isVertical
				? `transform: translateY(${y.current}px); top: 0; right: -3px; height: ${h.current}px; width: 2px;`
				: `transform: translateX(${x.current}px); bottom: -3px; left: 0; height: 2px; width: ${w.current}px;`}
		></span>
	{/if}
	{@render children?.()}
</TabsPrimitive.List>
