<script lang="ts">
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { Separator } from "$components/ui/separator";
  import * as Sidebar from "$components/ui/sidebar";
  import { config } from "$constants/app";
  let { children } = $props();
</script>

<Sidebar.Provider class="h-full min-h-full fixed inset-0" open={false}>
  <AppSidebar />
  <Sidebar.Inset class="@container/layout">
    <CustomTitlebar
      class="shrink-0 items-center gap-2 border-b border-border px-2"
    >
      <Sidebar.Trigger />
      <Separator
        orientation="vertical"
        class="me-2 data-[orientation=vertical]:h-3"
      />
      <div class="flex items-center gap-2 h-full" data-tauri-drag-region>
        <span class="text-sm font-semibold tracking-tight text-foreground"
          >{config.appName}</span>
      </div>
      <div class="flex-1 h-full" data-tauri-drag-region></div>
    </CustomTitlebar>
    <main class="flex-1 overflow-hidden no-scrollbar">
      {@render children()}
    </main>
  </Sidebar.Inset>
</Sidebar.Provider>
