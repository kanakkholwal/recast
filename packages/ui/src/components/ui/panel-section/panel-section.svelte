<script lang="ts" module>
import type { Snippet } from "svelte";

export interface PanelSectionProps {
	/** Section title — small uppercase label. Omit for header-less group. */
	title?: string;
	/** Tooltip content shown next to the title. Rendered via `hint` snippet
	 * if provided, otherwise as a `title` attribute on the label. */
	hint?: string;
	/** Custom hint renderer (e.g. an info icon with popover). */
	hintSlot?: Snippet;
	/** Right-aligned action slot (button, toggle, badge, count). */
	action?: Snippet;
	/** Body content. Optional — header-only sections are valid. */
	children?: Snippet;
	/** When true, child layout sets its own spacing. Default wraps body in
	 * a `space-y-2.5` group. */
	flush?: boolean;
	/** Make the section header click-to-toggle. Adds a chevron. */
	collapsible?: boolean;
	/** Initial open state when `collapsible`. Default true. */
	defaultOpen?: boolean;
	/** Controlled open state. When provided, overrides `defaultOpen`. */
	open?: boolean;
	/** Fired whenever the section toggles open/closed. */
	onOpenChange?: (open: boolean) => void;
	class?: string;
}
</script>

<script lang="ts">
	import { ChevronDown } from "@recast/icons";
	import { prefersReducedMotion, Spring } from "svelte/motion";
	import { slide } from "svelte/transition";
	import { cubicOut } from "svelte/easing";
	import { cn } from "@recast/ui/utils";

	let {
		title,
		hint,
		hintSlot,
		action,
		children,
		flush = false,
		collapsible = false,
		defaultOpen = true,
		open = $bindable<boolean | undefined>(undefined),
		onOpenChange,
		class: className,
	}: PanelSectionProps = $props();

	// Two modes:
	//   • Static (default): a labelled section with optional action — same
	//     visual as the legacy PanelSection.
	//   • Collapsible: header becomes a button that toggles a slide-animated
	//     body, with a spring-rotated chevron mirroring DialKit's Folder.

	const isControlled = $derived(open !== undefined);
	// svelte-ignore state_referenced_locally
	let internalOpen = $state(defaultOpen);

	const isOpen = $derived(isControlled ? !!open : internalOpen);

	// svelte-ignore state_referenced_locally
	const chevronRotation = new Spring(defaultOpen ? 0 : -90, {
		stiffness: 0.2,
		damping: 0.62,
	});

	// Both the spring and the body's slide run on JS/WAAPI, which the CSS
	// reduced-motion override in app.css never reaches.
	$effect(() => {
		chevronRotation.set(isOpen ? 0 : -90, { instant: prefersReducedMotion.current });
	});

	function toggle() {
		if (!collapsible) return;
		const next = !isOpen;
		if (!isControlled) internalOpen = next;
		else open = next;
		onOpenChange?.(next);
	}

	const hasHeader = $derived(!!title || !!action || collapsible);
</script>

<section class={cn("flex flex-col gap-2", className)}>
	{#if hasHeader}
		{#if collapsible}
			<button
				type="button"
				onclick={toggle}
				aria-expanded={isOpen}
				class={cn(
					"group/section flex min-h-5 items-center justify-between gap-2 rounded-sm outline-none transition-colors",
					"hover:text-foreground focus-visible:ring-2 focus-visible:ring-primary/30",
				)}
			>
				<div class="flex min-w-0 items-center gap-1.5">
					<span
						class="flex size-3 shrink-0 items-center justify-center text-muted-foreground/70 transition-colors group-hover/section:text-muted-foreground"
						aria-hidden="true"
						style:transform={`rotate(${chevronRotation.current}deg)`}
					>
						<ChevronDown class="size-3" />
					</span>
					{#if title}
						<span
							class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70 group-hover/section:text-muted-foreground"
							title={hint}
						>
							{title}
						</span>
					{/if}
					{#if hintSlot}{@render hintSlot()}{/if}
				</div>
				{#if action}
					<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
					<span
						role="presentation"
						onclick={(e) => e.stopPropagation()}
					>
						{@render action()}
					</span>
				{/if}
			</button>
		{:else}
			<header class="flex min-h-5 items-center justify-between gap-2">
				<div class="flex min-w-0 items-center gap-1.5">
					{#if title}
						<h3
							class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
							title={hint}
						>
							{title}
						</h3>
					{/if}
					{#if hintSlot}{@render hintSlot()}{/if}
				</div>
				{#if action}{@render action()}{/if}
			</header>
		{/if}
	{/if}

	{#if children}
		{#if collapsible}
			{#if isOpen}
				<div
					transition:slide={{
					duration: prefersReducedMotion.current ? 0 : 220,
					easing: cubicOut,
				}}
					style="clip-path: inset(0 -20px);"
				>
					{#if flush}
						{@render children()}
					{:else}
						<div class="space-y-2.5">{@render children()}</div>
					{/if}
				</div>
			{/if}
		{:else if flush}
			{@render children()}
		{:else}
			<div class="space-y-2.5">{@render children()}</div>
		{/if}
	{/if}
</section>
