<script lang="ts">
  import { isTauriApp } from "$lib/runtime/tauri";
  import { Minus, Square, X } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  interface Props {
    children?: Snippet;
    class?: string;
    wrapperClass?: string;
  }

  let { children, class: className, wrapperClass }: Props = $props();
  let isTauri = $state(false);
  let isMaximized = $state(false);

  onMount(async () => {
    isTauri = await isTauriApp();
    if (isTauri) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      isMaximized = await getCurrentWindow().isMaximized();
      getCurrentWindow().onResized(async () => {
        isMaximized = await getCurrentWindow().isMaximized();
      });
    }
  });

  async function handleMinimize(e: MouseEvent) {
    e.stopPropagation();
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  }

  async function handleToggleMaximize(e: MouseEvent) {
    e.stopPropagation();
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const maximized = await win.isMaximized();
      if (maximized) {
        await win.unmaximize();
      } else {
        await win.maximize();
      }
      isMaximized = !maximized;
    } catch (err) {
      console.error("Toggle maximize failed:", err);
    }
  }

  async function handleClose(e: MouseEvent) {
    e.stopPropagation();
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }
</script>

<div
  class={cn("h-10 flex items-center border-b border-border bg-background shrink-0 select-none", wrapperClass)}
>
  <!-- Drag region: only the content area, not the window controls -->
  <div class={cn("flex-1 flex items-center min-w-0 h-full", className)} data-tauri-drag-region>
    {#if children}
      {@render children()}
    {/if}
  </div>

  <!-- Window controls: outside the drag region so clicks aren't intercepted -->
  {#if isTauri}
    <div class="shrink-0 flex items-center h-full">
      <Button
        variant="ghost"
        size="raw"
        onmousedown={(e) => e.stopPropagation()}
        onclick={handleMinimize}
        class="h-full w-9 rounded-none text-muted-foreground hover:bg-muted"
        aria-label="Minimize"
        title="Minimize"
      >
        <Minus size={12} strokeWidth={1.5} />
      </Button>
      <Button
        variant="ghost"
        size="raw"
        onmousedown={(e) => e.stopPropagation()}
        onclick={handleToggleMaximize}
        class="h-full w-9 rounded-none text-muted-foreground hover:bg-muted"
        aria-label={isMaximized ? "Restore" : "Maximize"}
        title={isMaximized ? "Restore" : "Maximize"}
      >
        {#if isMaximized}
          <svg width="11" height="11" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="3" y="0.5" width="9" height="9" rx="1.5" />
            <rect x="0.5" y="3" width="9" height="9" rx="1.5" />
          </svg>
        {:else}
          <Square size={11} strokeWidth={1.5} />
        {/if}
      </Button>
      <Button
        variant="raw"
        size="raw"
        onmousedown={(e) => e.stopPropagation()}
        onclick={handleClose}
        class="h-full w-9 rounded-none text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
        aria-label="Close"
        title="Close"
      >
        <X size={13} strokeWidth={1.5} />
      </Button>
    </div>
  {/if}
</div>
