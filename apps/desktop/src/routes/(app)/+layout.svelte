<script lang="ts">
import { page } from "$app/state";
import UploadDialogsHost from "$components/cloud/UploadDialogsHost.svelte";
import CornerNotifications from "$components/corner-notifications.svelte";
import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
import TopNav from "$components/layout/TopNav.svelte";
import WhatsNewDialog from "$components/whats-new-dialog.svelte";
import { config } from "$constants/app";
import { updater } from "$lib/stores/updater.svelte";
import { whatsNew } from "$lib/stores/whats-new.svelte";
import { onMount } from "svelte";

let { children } = $props();
let section = $derived(
	page.url.pathname === "/" ? "Home" : page.url.pathname.replace(/^\//, "").split("/")[0],
);

onMount(() => {
	if (page.url.pathname.startsWith("/whats-new")) whatsNew.markSeen();
	else whatsNew.evaluateOnBoot();
	updater.init();
});
</script>

<div class="fixed inset-0 flex min-h-0 flex-col bg-background">
  <CustomTitlebar class="items-center gap-1 px-3">
    <span
      class="pointer-events-none select-none text-[13px] font-semibold tracking-tight text-foreground/70"
      data-tauri-drag-region
    >
      {config.appName}
    </span>
    <span
      class="pointer-events-none select-none text-[11px] font-medium text-muted-foreground/50"
      data-tauri-drag-region
    >
      ·
    </span>
    <span
      class="pointer-events-none select-none truncate text-[11px] font-medium capitalize text-muted-foreground/70"
      data-tauri-drag-region
    >
      {section}
    </span>
    <div class="h-full flex-1" data-tauri-drag-region></div>
  </CustomTitlebar>

  <TopNav />

  <main class="no-scrollbar min-h-0 flex-1 overflow-hidden bg-background pt-2">
    {@render children()}
  </main>
</div>

<WhatsNewDialog />
<CornerNotifications />
<UploadDialogsHost />
