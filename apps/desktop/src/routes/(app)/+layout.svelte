<script lang="ts">
  import { page } from "$app/state";
  import CornerNotifications from "$components/corner-notifications.svelte";
  import ActivityCenter from "$components/layout/activity-center.svelte";
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import WindowControls from "$components/layout/window-controls.svelte";
  import WhatsNewDialog from "$components/whats-new-dialog.svelte";
  import { config } from "$constants/app";
  import { shortcutsDialog } from "$lib/shortcuts/registry.svelte";
  import { layoutMode } from "$lib/stores/layout-mode.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { whatsNew } from "$lib/stores/whats-new.svelte";
  import { Keyboard } from "@lucide/svelte";
  import * as Sidebar from "@recast/ui/sidebar";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";

  let { children } = $props();
  let routeKey = $derived(page.url.pathname);
  let section = $derived(
    routeKey === "/" ? "Home" : routeKey.replace(/^\//, "").split("/")[0],
  );

  // Detected synchronously from the UA so there's no chrome flash on first
  // paint; false under SSR.
  const isMac =
    typeof navigator !== "undefined" &&
    /mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent || "");

  // Only the titlebar differs by mode: `os-native` lifts a full-width window
  // titlebar above the shell with OS-placed controls; `recast` embeds it in the
  // content header.
  let osNative = $derived(layoutMode.current === "os-native");

  onMount(() => {
    // Surface "What's new" once per release (skip if already on the changelog).
    if (page.url.pathname.startsWith("/whats-new")) {
      whatsNew.markSeen();
    } else {
      whatsNew.evaluateOnBoot();
    }
    updater.init();
  });
</script>

<!-- In `recast` mode this also carries the embedded titlebar at the top. -->
{#snippet shell()}
  <AppSidebar variant="inset" />
  <Sidebar.Inset
    class="@container/layout overflow-hidden md:peer-data-[variant=inset]:ml-0"
  >
    {#if !osNative}
      <CustomTitlebar class="items-center gap-1 px-3">
        <div
          class="flex h-full items-center gap-2 font-sans"
          
        >
          <div
            in:fade={{ duration: 180, delay: 100, easing: cubicOut }}
            out:fade={{ duration: 140, easing: cubicOut }}
          >
            <Sidebar.Trigger
              class="size-7 rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
              title="Pin / unpin sidebar (⌘B)"
            />
          </div>
          <span
            class="pointer-events-none select-none text-[13px] font-semibold tracking-tight text-foreground/80"
            data-tauri-drag-region
          >
            {config.appName}
          </span>
          <span
            class="pointer-events-none select-none text-[11px] font-medium text-muted-foreground/60"
            data-tauri-drag-region
          >
            ·
          </span>
          <span
            class="pointer-events-none select-none truncate text-[11px] font-medium capitalize text-muted-foreground/80"
            data-tauri-drag-region
          >
            {section}
          </span>
        </div>
        <div class="h-full flex-1" data-tauri-drag-region></div>
      </CustomTitlebar>
    {/if}

    <main class="no-scrollbar flex-1 overflow-hidden">
      <div class="h-full">
        {@render children()}
      </div>
    </main>
  </Sidebar.Inset>
{/snippet}

{#if osNative}
  <!-- Inside the provider so the sidebar trigger has context, but `fixed top-0`
       so it renders as a real window titlebar above the shell. The whole bar is
       a drag region; buttons opt out. `.os-native-shell` (app.css) offsets the
       fixed sidebar down by the titlebar height. -->
  <Sidebar.Provider
    class="os-native-shell fixed inset-x-0 bottom-0 top-10 min-h-0"
  >
    <header
      data-recast-titlebar
      class="bg-sidebar fixed inset-x-0 top-0 z-50 flex h-10 select-none items-center gap-1 px-3"
    >
      {#if isMac}
        <WindowControls kind="mac" />
      {/if}
      <div
        class="flex h-full items-center gap-2 font-sans"
        
      >
        <Sidebar.Trigger
          class="size-7 rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
          title="Pin / unpin sidebar (⌘B)"
        />
        <span
          class="pointer-events-none select-none text-[13px] font-semibold tracking-tight text-foreground/80"
          data-tauri-drag-region
        >
          {config.appName}
        </span>
        <span
          class="pointer-events-none select-none text-[11px] font-medium text-muted-foreground/60"
          data-tauri-drag-region
        >
          ·
        </span>
        <span
          class="pointer-events-none select-none truncate text-[11px] font-medium capitalize text-muted-foreground/80"
          data-tauri-drag-region
        >
          {section}
        </span>
      </div>
      <div class="h-full flex-1" data-tauri-drag-region></div>
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
      {#if !isMac}
        <WindowControls kind="win" class="shrink-0" />
      {/if}
    </header>

    {@render shell()}
  </Sidebar.Provider>
{:else}
  <Sidebar.Provider class="fixed inset-0 h-full min-h-full">
    {@render shell()}
  </Sidebar.Provider>
{/if}

<WhatsNewDialog />
<CornerNotifications />
