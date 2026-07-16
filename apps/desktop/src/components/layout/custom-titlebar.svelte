<script lang="ts">
  import ActivityCenter from "$components/layout/activity-center.svelte";
  import WindowControls from "$components/layout/window-controls.svelte";
  import { shortcutsDialog } from "$lib/shortcuts/registry.svelte";
  import { layoutMode } from "$lib/stores/layout-mode.svelte";
  import { Keyboard } from "@recast/icons";
  import { cn } from "@recast/ui/utils";
  import { platform } from "@tauri-apps/plugin-os";
  import type { Snippet } from "svelte";

  interface Props {
    children?: Snippet;
    class?: string;
    wrapperClass?: string;
  }

  let { children, class: className, wrapperClass }: Props = $props();

  // Synchronous so there's no chrome flash on first paint; false under SSR.
  const isMac = ["darwin", "ios"].includes(platform());
  // os-native on macOS → traffic lights lead left; otherwise min/max/close right.
  const macLights = $derived(layoutMode.current === "os-native" && isMac);
</script>

<div
  data-recast-titlebar
  class={cn(
    "group h-10 flex items-center gap-1 border-b border-border/60 bg-background/70 backdrop-blur-xl shrink-0 select-none px-1 py-1 transition-all duration-300",
    wrapperClass,
  )}
>
  <!-- macOS · os-native: traffic lights lead the bar, before the content. -->
  {#if macLights}
    <WindowControls kind="mac" class="px-1.5" />
  {/if}

  <!-- Drag region: content area only, not the window controls. -->
  <div
    class={cn("flex-1 flex items-center min-w-0 h-full font-sans", className)}
    
  >
    {#if children}
      {@render children()}
    {/if}
  </div>

  <!-- Outside the drag region so clicks register. -->
  <ActivityCenter />

  <button
    type="button"
    onclick={() => shortcutsDialog.show()}
    onmousedown={(e) => e.stopPropagation()}
    aria-label="Keyboard shortcuts"
    title="Keyboard shortcuts (Ctrl + /)"
    class="group inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
  >
    <Keyboard size={15} />
  </button>

  <!-- Outside the drag region so clicks aren't intercepted. -->
  {#if !macLights}
    <WindowControls kind="win" class="shrink-0" />
  {/if}
</div>
