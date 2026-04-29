<script lang="ts">
  import { page } from "$app/state";
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { config } from "$constants/app";
  import { Separator } from "@recast/ui/separator";
  import * as Sidebar from "@recast/ui/sidebar";

  let { children } = $props();

  let routeKey = $derived(page.url.pathname);
</script>

<Sidebar.Provider class="h-full min-h-full fixed inset-0" open={false}>
  <AppSidebar variant="floating" />
  <Sidebar.Inset class="@container/layout">
    <CustomTitlebar class="items-center gap-1 px-1.5">
      <div
        class="flex items-center gap-1.5 opacity-40 transition-opacity duration-300 hover:opacity-100"
      >
        <Sidebar.Trigger
          class="size-7 rounded-[10px] transition-colors hover:bg-foreground/5"
        />
        <Separator
          orientation="vertical"
          class="data-[orientation=vertical]:h-3 opacity-20"
        />
      </div>
      <div
        class="flex h-full items-center gap-2 pl-1.5 font-sans"
        data-tauri-drag-region
      >
        <span
          class="pointer-events-none select-none text-[13px] font-semibold tracking-tight text-foreground/80"
          >{config.appName}</span
        >
      </div>
      <div class="h-full flex-1" data-tauri-drag-region></div>
    </CustomTitlebar>
    <main class="flex-1 overflow-hidden no-scrollbar">
      {#key routeKey}
        <div class="h-full animate-fade-in motion-reduce:animate-none">
          {@render children()}
        </div>
      {/key}
    </main>
  </Sidebar.Inset>
</Sidebar.Provider>
