<script lang="ts">
  import { page } from "$app/state";
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { config } from "$constants/app";
  import * as Sidebar from "@recast/ui/sidebar";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  let { children } = $props();
  let routeKey = $derived(page.url.pathname);

  const PIN_STORAGE_KEY = "recast.sidebar.pinned";

  // Two independent triggers control the sidebar's visible state:
  //   pinned  — sticky, toggled by Sidebar.Trigger / ⌘B (persisted)
  //   hovered — transient, set by pointer-enter/leave on the sidebar rail
  // Effective open is the OR of both, so:
  //   • Hovering the collapsed rail peeks the sidebar open.
  //   • The trigger pins it open so it survives mouse-leave.
  //   • Clicking the trigger again unpins; if you're still hovering, it stays
  //     peeked until the cursor leaves.
  let pinned = $state(false);
  let hovered = $state(false);
  let leaveTimer: number | null = null;

  $effect.pre(() => {
    if (typeof window === "undefined") return;
    pinned = window.localStorage.getItem(PIN_STORAGE_KEY) === "true";
  });
  $effect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(PIN_STORAGE_KEY, String(pinned));
  });

  const sidebarOpen = $derived(pinned || hovered);

  function handleEnter() {
    if (leaveTimer) {
      window.clearTimeout(leaveTimer);
      leaveTimer = null;
    }
    hovered = true;
  }
  function handleLeave() {
    if (leaveTimer) window.clearTimeout(leaveTimer);
    // Small grace period so brief gap-crossings (e.g., onto a tooltip) don't
    // collapse the panel mid-interaction.
    leaveTimer = window.setTimeout(() => {
      hovered = false;
      leaveTimer = null;
    }, 140);
  }
</script>

<Sidebar.Provider
  class="h-full min-h-full fixed inset-0"
  open={sidebarOpen}
  onOpenChange={() => (pinned = !pinned)}
>
  <AppSidebar onpointerenter={handleEnter} onpointerleave={handleLeave} />
  <Sidebar.Inset class="@container/layout">
    <CustomTitlebar class="items-center gap-1 px-3">
      <div
        class="flex h-full items-center gap-2 font-sans"
        data-tauri-drag-region
      >
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
          {routeKey === "/" ? "Home" : routeKey.replace(/^\//, "").split("/")[0]}
        </span>
      </div>
      <div class="h-full flex-1" data-tauri-drag-region></div>
    </CustomTitlebar>
    <main class="flex-1 overflow-hidden no-scrollbar">
      {#key routeKey}
        <div
          in:fly={{ y: 6, duration: 240, easing: cubicOut, delay: 60 }}
          out:fade={{ duration: 120 }}
          class="h-full motion-reduce:animate-none"
        >
          {@render children()}
        </div>
      {/key}
    </main>
  </Sidebar.Inset>
</Sidebar.Provider>
