<script lang="ts">
  import { isTauriApp } from "$lib/runtime/tauri";
  import { Minus, Square, X } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  interface Props {
    children?: Snippet;
  }

  let { children }: Props = $props();
  let isTauri = $state(false);
  let isMaximized = $state(false);

  onMount(async () => {
    isTauri = await isTauriApp();
    if (isTauri) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      isMaximized = await getCurrentWindow().isMaximized();
      // Listen for maximize/unmaximize changes.
      getCurrentWindow().onResized(async () => {
        isMaximized = await getCurrentWindow().isMaximized();
      });
    }
  });

  async function minimize() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  }

  async function toggleMaximize() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().toggleMaximize();
  }

  async function close() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }
</script>

<div
  class="h-10 flex items-center border-b border-border bg-background shrink-0 select-none"
  data-tauri-drag-region
>
  <div class="flex-1 flex items-center min-w-0 h-full" data-tauri-drag-region>
    {#if children}
      {@render children()}
    {/if}
  </div>

  {#if isTauri}
    <div class="shrink-0 flex items-center h-full">
      <button
        onclick={minimize}
        class="h-full w-11 flex items-center justify-center text-muted-foreground hover:bg-muted transition-colors"
        aria-label="Minimize"
      >
        <Minus size={14} strokeWidth={1.5} />
      </button>
      <button
        onclick={toggleMaximize}
        class="h-full w-11 flex items-center justify-center text-muted-foreground hover:bg-muted transition-colors"
        aria-label={isMaximized ? "Restore" : "Maximize"}
      >
        {#if isMaximized}
          <!-- Restore icon: two overlapping squares -->
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="3" y="0.5" width="9" height="9" rx="1.5" />
            <rect x="0.5" y="3" width="9" height="9" rx="1.5" />
          </svg>
        {:else}
          <Square size={13} strokeWidth={1.5} />
        {/if}
      </button>
      <button
        onclick={close}
        class="h-full w-11 flex items-center justify-center text-muted-foreground hover:bg-red-500 hover:text-white transition-colors"
        aria-label="Close"
      >
        <X size={15} strokeWidth={1.5} />
      </button>
    </div>
  {/if}
</div>
