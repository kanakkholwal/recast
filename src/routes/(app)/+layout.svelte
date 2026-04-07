<script lang="ts">
  import AppSidebar from "$components/layout/app-sidebar.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { Separator } from "$components/ui/separator";
  import * as Sidebar from "$components/ui/sidebar";
  import { config } from "$constants/app";
  let { children } = $props();
</script>

<div class="flex flex-col h-screen">
  <CustomTitlebar>
    <div class="flex items-center gap-2 px-3 h-full" data-tauri-drag-region>
      <span class="text-sm font-semibold text-foreground">{config.appName}</span>
    </div>
  </CustomTitlebar>

  <div class="flex-1 min-h-0">
    <Sidebar.Provider>
      <AppSidebar />
      <Sidebar.Inset class="@container/layout">
        <header class="flex h-12 shrink-0 items-center gap-2 border-b border-border">
          <div class="flex items-center gap-2 px-4">
            <Sidebar.Trigger class="-ms-1" />
            <Separator orientation="vertical" class="me-2 data-[orientation=vertical]:h-4" />
          </div>
        </header>
        {@render children()}
      </Sidebar.Inset>
    </Sidebar.Provider>
  </div>
</div>
