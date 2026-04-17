<script lang="ts">
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { config } from "$constants/app";
  import { Separator } from "@recast/ui/separator";
  import * as Sidebar from "@recast/ui/sidebar";

  let { children } = $props();
</script>

<Sidebar.Provider class="h-full min-h-full fixed inset-0" open={false}>
  <AppSidebar variant="floating" />
  <Sidebar.Inset class="@container/layout">
    <CustomTitlebar class="items-center gap-1 px-1.5">
      <div
        class="flex items-center gap-1.5 opacity-40 hover:opacity-100 transition-opacity duration-300"
      >
        <Sidebar.Trigger class="size-7 rounded-[10px] hover:bg-foreground/5" />
        <Separator
          orientation="vertical"
          class="data-[orientation=vertical]:h-3 opacity-20"
        />
      </div>
      <div
        class="flex items-center gap-2 h-full font-sans pl-1.5"
        data-tauri-drag-region
      >
        <span
          class="text-[13px] font-semibold tracking-tight text-foreground/80 pointer-events-none select-none"
          >{config.appName}</span
        >
      </div>
      <div class="flex-1 h-full" data-tauri-drag-region></div>
    </CustomTitlebar>
    <main class="flex-1 overflow-hidden no-scrollbar">
      {@render children()}
    </main>
  </Sidebar.Inset>
</Sidebar.Provider>
