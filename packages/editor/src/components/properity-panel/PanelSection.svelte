<script lang="ts">
import { ChevronDown } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import { cubicOut } from "svelte/easing";
import { prefersReducedMotion, Spring } from "svelte/motion";
import { slide } from "svelte/transition";
import InspectorHint from "../InspectorHint.svelte";

// The inspector section header. Figma/Premiere register: a normal-case label
interface Props {
	/** Section title. Omit to render a header-less group. */
	title?: string;
	/** Explanatory tooltip rendered next to the title. */
	hint?: string;
	/** Right-aligned action slot (button, toggle, badge, count). */
	action?: Snippet;
	/** Body content. Optional, so header-only sections are valid. */
	children?: Snippet;
	/** When true, child layout sets its own spacing. Default wraps in `space-y-2.5`. */
	flush?: boolean;
	/** Make the section collapsible with a chevron + slide. */
	collapsible?: boolean;
	/** Initial open state when `collapsible`. Default true. */
	defaultOpen?: boolean;
	/** Controlled open state. */
	open?: boolean;
	onOpenChange?: (open: boolean) => void;
	class?: string;
}

let {
	title,
	hint,
	action,
	children,
	flush = false,
	collapsible = false,
	defaultOpen = true,
	open = $bindable<boolean | undefined>(undefined),
	onOpenChange,
	class: className,
}: Props = $props();

const isControlled = $derived(open !== undefined);
// svelte-ignore state_referenced_locally
let internalOpen = $state(defaultOpen);
const isOpen = $derived(isControlled ? open === true : internalOpen);

// svelte-ignore state_referenced_locally
const chevronRotation = new Spring(defaultOpen ? 0 : -90, {
	stiffness: 0.2,
	damping: 0.62,
});

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

const hasHeader = $derived(title !== undefined || action !== undefined || collapsible);
const labelClass =
	"truncate text-[13px] font-semibold tracking-tight text-foreground transition-colors";
</script>

<section class={cn("flex flex-col gap-2", className)}>
  {#if hasHeader}
    {#if collapsible}
      <div class="flex min-h-6 items-center justify-between gap-2">
        <button
          type="button"
          onclick={toggle}
          aria-expanded={isOpen}
          class="group/section flex min-w-0 flex-1 items-center gap-1.5 rounded-md text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        >
          <span
            class="flex size-3 shrink-0 items-center justify-center text-muted-foreground/50 transition-colors group-hover/section:text-muted-foreground"
            aria-hidden="true"
            style:transform={`rotate(${chevronRotation.current}deg)`}
          >
            <ChevronDown class="size-3" />
          </span>
          {#if title}
            <span class="{labelClass} group-hover/section:text-foreground">{title}</span>
          {/if}
          {#if hint}<InspectorHint content={hint} />{/if}
        </button>
        {#if action}<div class="shrink-0">{@render action()}</div>{/if}
      </div>
    {:else}
      <header class="flex min-h-6 items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-1.5">
          {#if title}<h3 class={labelClass}>{title}</h3>{/if}
          {#if hint}<InspectorHint content={hint} />{/if}
        </div>
        {#if action}<div class="shrink-0">{@render action()}</div>{/if}
      </header>
    {/if}
  {/if}

  {#if children}
    {#if collapsible}
      {#if isOpen}
        <div
          transition:slide={{ duration: prefersReducedMotion.current ? 0 : 220, easing: cubicOut }}
          style="clip-path: inset(0 -20px);"
        >
          {#if flush}{@render children()}{:else}<div class="space-y-2">{@render children()}</div>{/if}
        </div>
      {/if}
    {:else if flush}
      {@render children()}
    {:else}
      <div class="space-y-2">{@render children()}</div>
    {/if}
  {/if}
</section>
