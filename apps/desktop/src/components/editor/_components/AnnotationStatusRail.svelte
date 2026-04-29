<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { Eye, EyeOff, Layers, Pencil } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  // Auto-hide when there are zero annotations so non-annotation users never see
  // the rail. Visible whenever any annotation exists, regardless of which tab
  // the user is on — that's the whole point of a global status rail.
  const visible = $derived(store.annotations.length > 0);
  const onAnnotationsTab = $derived(store.activePanel === "annotations");
  const hidden = $derived(store.annotationsGloballyHidden);

  function openAnnotationsTab() {
    store.activePanel = "annotations";
  }

  function toggleHide() {
    store.annotationsGloballyHidden = !store.annotationsGloballyHidden;
  }
</script>

{#if visible}
  <div
    class={cn(
      "pointer-events-none absolute inset-x-0 top-3 z-30 flex justify-center px-3",
      "animate-fade-in motion-reduce:animate-none",
    )}
  >
    <div
      class={cn(
        "pointer-events-auto flex items-center gap-1 rounded-full border border-border-low bg-background/85 px-1.5 py-1 shadow-craft-sm backdrop-blur-md",
        "shadow-(--shadow-craft-inset)",
      )}
    >
      <Button
        variant="ghost"
        size="xs"
        onclick={openAnnotationsTab}
        class="gap-1.5 rounded-full px-2.5"
        title="Open annotation panel"
      >
        <Layers size={12} class="text-muted-foreground" />
        <span class="text-[11px] font-semibold tabular-nums text-foreground">
          {store.annotations.length}
        </span>
        <span class="text-[11px] font-medium text-muted-foreground">
          {store.annotations.length === 1 ? "layer" : "layers"}
        </span>
      </Button>

      <span class="mx-0.5 h-3 w-px bg-border-low" aria-hidden="true"></span>

      <Button
        variant="ghost"
        size="icon-xs"
        onclick={toggleHide}
        class={cn(
          "rounded-full transition-colors",
          hidden && "text-warning",
        )}
        title={hidden ? "Show all annotations" : "Hide all annotations"}
        aria-pressed={hidden}
      >
        {#if hidden}
          <EyeOff size={12} />
        {:else}
          <Eye size={12} />
        {/if}
      </Button>

      {#if onAnnotationsTab}
        <span class="mx-0.5 h-3 w-px bg-border-low" aria-hidden="true"></span>
        <span
          class={cn(
            "inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5",
            "text-[10px] font-semibold uppercase tracking-[0.12em] text-primary",
          )}
        >
          <Pencil size={10} />
          Editing
        </span>
      {/if}
    </div>
  </div>
{/if}
