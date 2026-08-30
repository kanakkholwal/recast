<script lang="ts" module>
export type ExportPanelPhase =
	| "options"
	| "queued"
	| "progress"
	| "success"
	| "cancelled"
	| "error";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";
  import { isOverlayOpen } from "../lib/dom/keyboard";

  // Inline right-rail export surface: each phase brings its own header and footer, so this is a scroll host that crossfades.
  interface Props {
    phase: ExportPanelPhase | null;
    onEscape?: () => void;
    options?: Snippet;
    queued?: Snippet;
    progress?: Snippet;
    success?: Snippet;
    cancelled?: Snippet;
    error?: Snippet;
  }

  let {
    phase,
    onEscape,
    options,
    queued,
    progress,
    success,
    cancelled,
    error,
  }: Props = $props();

  // Escape leaves the export flow, but only when nothing is stacked on top: a dialog or Select owns Escape while open.
  function handleKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    if (e.defaultPrevented || isOverlayOpen()) return;
    e.preventDefault();
    onEscape?.();
  }

  // Focus moves in but is not trapped: this is a focused task, not a modal, since the live preview stays beside it.
  let sectionEl = $state<HTMLElement | null>(null);
  onMount(() => {
    // Skip if a phase already placed focus on one of its own fields.
    if (sectionEl && !sectionEl.contains(document.activeElement)) {
      sectionEl.focus({ preventScroll: true });
    }
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<section
  bind:this={sectionEl}
  tabindex="-1"
  aria-label="Export"
  class="@container/export relative h-full min-h-0 w-full overflow-hidden bg-background focus:outline-none"
>
  <!-- Each phase fills the rail so its own footer can pin to the bottom while
       the body scrolls. Phases are absolutely stacked so they crossfade in
       place rather than shoving each other during the swap. -->
  {#key phase}
    <div
      class="absolute inset-0 flex flex-col"
      in:fade={{ duration: 180, delay: 120, easing: cubicOut }}
      out:fade={{ duration: 120, easing: cubicOut }}
    >
      {#if phase === "options"}
        {@render options?.()}
      {:else if phase === "queued"}
        {@render queued?.()}
      {:else if phase === "progress"}
        {@render progress?.()}
      {:else if phase === "success"}
        {@render success?.()}
      {:else if phase === "cancelled"}
        {@render cancelled?.()}
      {:else if phase === "error"}
        {@render error?.()}
      {/if}
    </div>
  {/key}
</section>
