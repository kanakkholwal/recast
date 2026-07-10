<script lang="ts">
  import type { Snippet } from "svelte";
  import { PanelSection } from "@recast/ui/panel-section";
  import InspectorHint from "../InspectorHint.svelte";

  interface Props {
    /** Section title: small uppercase label. Omit to render a header-less group. */
    title?: string;
    /** Optional explanatory tooltip rendered next to the title. */
    hint?: string;
    /** Right-aligned action slot (button, toggle, badge, count). */
    action?: Snippet;
    /** Body content. Optional, so header-only sections (e.g. just a toggle) are valid. */
    children?: Snippet;
    /** When true, child layout sets its own spacing. Default wraps in a `space-y-2.5` group. */
    flush?: boolean;
    /** Make the section collapsible with chevron + slide. */
    collapsible?: boolean;
    /** Initial open state when `collapsible`. Default true. */
    defaultOpen?: boolean;
    /** Controlled open state. */
    open?: boolean;
    onOpenChange?: (open: boolean) => void;
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
  }: Props = $props();
</script>

<PanelSection
  {title}
  {action}
  {flush}
  {collapsible}
  {defaultOpen}
  bind:open
  {onOpenChange}
>
  {#snippet hintSlot()}
    {#if hint}
      <InspectorHint content={hint} />
    {/if}
  {/snippet}
  {#if children}{@render children()}{/if}
</PanelSection>
