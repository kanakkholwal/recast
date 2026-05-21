<script lang="ts">
  import { page } from "$app/state";
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CornerNotifications from "$components/corner-notifications.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import WhatsNewDialog from "$components/whats-new-dialog.svelte";
  import { config } from "$constants/app";
  import { updater } from "$lib/stores/updater.svelte";
  import { whatsNew } from "$lib/stores/whats-new.svelte";
  import * as Sidebar from "@recast/ui/sidebar";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";

  let { children } = $props();
  let routeKey = $derived(page.url.pathname);

  // On boot: surface the "What's new" corner card once per release (skip if
  // the user already landed on the changelog page), and kick off the
  // background update check. Both render as non-blocking bottom-right cards.
  onMount(() => {
    if (page.url.pathname.startsWith("/whats-new")) {
      whatsNew.markSeen();
    } else {
      whatsNew.evaluateOnBoot();
    }
    updater.init();
  });
</script>

<Sidebar.Provider class="h-full min-h-full fixed inset-0">
  <AppSidebar />
  <Sidebar.Inset class="@container/layout">
    <CustomTitlebar class="items-center gap-1 px-3">
      <div
        class="flex h-full items-center gap-2 font-sans"
        data-tauri-drag-region
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
          class="pointer-events-none select-none truncate text-[11px] font-medium text-muted-foreground/80"
          data-tauri-drag-region
        >
          {routeKey === "/"
            ? "Home"
            : routeKey.replace(/^\//, "").split("/")[0]}
        </span>
      </div>
      <div class="h-full flex-1" data-tauri-drag-region></div>
    </CustomTitlebar>
    <main class="flex-1 overflow-hidden no-scrollbar">
      <div class="h-full">
        {@render children()}
      </div>
    </main>
  </Sidebar.Inset>
</Sidebar.Provider>

<WhatsNewDialog />
<CornerNotifications />
