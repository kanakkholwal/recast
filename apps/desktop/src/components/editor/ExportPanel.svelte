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
  import { isOverlayOpen } from "$lib/dom/keyboard";

  // Inline right-rail export surface: the same phase snippets that used to live
  // in a portaled modal, re-homed where the properties panel was so the live
  // preview stays mounted beside it (no overlay covering the video). Each phase
  // brings its own header/footer, so this is just a scroll host that crossfades
  // between phases.
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

  // Only mounted while a phase is active, so Escape here means "leave the export
  // flow" (cancel a run, dismiss a result, or close the picker) -- but ONLY when
  // nothing is stacked on top of us. A dialog, a menu, or a Select inside the
  // options form owns Escape while it's open; without the guard, dismissing one
  // falls through to here and cancels the export behind it.
  function handleKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    if (e.defaultPrevented || isOverlayOpen()) return;
    e.preventDefault();
    onEscape?.();
  }

  // Move focus into the panel when the export flow opens, so keyboard and screen
  // reader users are taken to the new task rather than left on the toolbar button
  // behind it. The panel is a focused task (own Esc-to-leave, blocks editor
  // shortcuts) but deliberately NOT a modal: the live preview stays beside it, so
  // we move focus without trapping it. The route restores focus to the trigger on
  // close. tabindex=-1 makes the region focusable without joining the tab order.
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
