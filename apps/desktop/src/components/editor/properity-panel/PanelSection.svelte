<script lang="ts">
  import type { Snippet } from "svelte";
  import InspectorHint from "../InspectorHint.svelte";

  interface Props {
    /** Section title — small uppercase label. Omit to render a header-less group. */
    title?: string;
    /** Optional explanatory tooltip rendered next to the title. */
    hint?: string;
    /** Right-aligned action slot (button, toggle, badge, count). */
    action?: Snippet;
    /** Body content. Optional — header-only sections (e.g. just a toggle) are valid. */
    children?: Snippet;
    /** When true, child layout sets its own spacing. Default wraps in a `space-y-2.5` group. */
    flush?: boolean;
  }

  let { title, hint, action, children, flush = false }: Props = $props();
  const hasHeader = $derived(!!title || !!action);
</script>

<section class="flex flex-col gap-2">
  {#if hasHeader}
    <header class="flex min-h-5 items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-1.5">
        {#if title}
          <h3
            class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            {title}
          </h3>
        {/if}
        {#if hint}
          <InspectorHint content={hint} />
        {/if}
      </div>
      {#if action}{@render action()}{/if}
    </header>
  {/if}

  {#if children}
    {#if flush}
      {@render children()}
    {:else}
      <div class="space-y-2.5">{@render children()}</div>
    {/if}
  {/if}
</section>
