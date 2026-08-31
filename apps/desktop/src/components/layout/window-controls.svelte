<script module lang="ts">
// Flat, native-style Windows control: a wide hit target, no pill container.
const winBtn =
	"group cursor-pointer inline-flex h-7 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground active:scale-95";
</script>

<script lang="ts">
  import { isTauriApp } from "$lib/runtime/tauri";
  import { Minus, Plus, Square, X } from "@recast/icons";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";

  // `mac` draws faux traffic lights and `win` draws min/max/close; the caller picks the variant via platform().
  let { kind, class: className }: { kind: "mac" | "win"; class?: string } =
    $props();

  let isTauri = $state(false);
  let isMaximized = $state(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    void (async () => {
      isTauri = await isTauriApp();
      if (!isTauri) return;
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      isMaximized = await win.isMaximized();
      // Keep the maximize glyph in sync with OS-driven resizes such as snap or a titlebar double-click.
      unlisten = await win.onResized(async () => {
        isMaximized = await win.isMaximized();
      });
    })();
    return () => unlisten?.();
  });

  async function currentWindow() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }
  async function minimize(e: MouseEvent) {
    e.stopPropagation();
    (await currentWindow()).minimize();
  }
  async function toggleMaximize(e: MouseEvent) {
    e.stopPropagation();
    const win = await currentWindow();
    if (await win.isMaximized()) await win.unmaximize();
    else await win.maximize();
  }
  async function close(e: MouseEvent) {
    e.stopPropagation();
    (await currentWindow()).close();
  }
</script>

{#if isTauri}
  {#if kind === "mac"}

    <div
      class={cn("group/lights flex items-center gap-2", className)}
      onmousedown={(e) => e.stopPropagation()}
      role="presentation"
    >
      <button
        type="button"
        onclick={close}
        aria-label="Close"
        title="Close"
        class="cursor-pointer inline-flex size-3 items-center justify-center rounded-full bg-[#ff5f57] ring-1 ring-inset ring-black/15"
      >
        <X
          size={8}
          stroke={2.5}
          class="text-black/55 opacity-0 transition-opacity group-hover/lights:opacity-100"
        />
      </button>
      <button
        type="button"
        onclick={minimize}
        aria-label="Minimize"
        title="Minimize"
        class="cursor-pointer inline-flex size-3 items-center justify-center rounded-full bg-[#febc2e] ring-1 ring-inset ring-black/15"
      >
        <Minus
          size={8}
          stroke={2.5}
          class="text-black/55 opacity-0 transition-opacity group-hover/lights:opacity-100"
        />
      </button>
      <button
        type="button"
        onclick={toggleMaximize}
        aria-label={isMaximized ? "Restore" : "Zoom"}
        title={isMaximized ? "Restore" : "Zoom"}
        class="cursor-pointer inline-flex size-3 items-center justify-center rounded-full bg-[#28c840] ring-1 ring-inset ring-black/15"
      >
        <Plus
          size={8}
          stroke={2.5}
          class="text-black/55 opacity-0 transition-opacity group-hover/lights:opacity-100"
        />
      </button>
    </div>
  {:else}
    <div
      class={cn("inline-flex items-center gap-0.5", className)}
      onmousedown={(e) => e.stopPropagation()}
      role="presentation"
    >
      <button
        type="button"
        onclick={minimize}
        aria-label="Minimize"
        title="Minimize"
        class={winBtn}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        onclick={toggleMaximize}
        aria-label={isMaximized ? "Restore" : "Maximize"}
        title={isMaximized ? "Restore" : "Maximize"}
        class={winBtn}
      >
        {#if isMaximized}
          <svg
            width="14"
            height="14"
            viewBox="0 0 13 13"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          >
            <rect x="3" y="0.5" width="9" height="9" rx="1.5" />
            <rect x="0.5" y="3" width="9" height="9" rx="1.5" />
          </svg>
        {:else}
          <Square size={14} />
        {/if}
      </button>
      <button
        type="button"
        onclick={close}
        aria-label="Close"
        title="Close"
        class={cn(winBtn, "hover:bg-destructive hover:text-destructive-foreground")}
      >
        <X size={16} />
      </button>
    </div>
  {/if}
{/if}
