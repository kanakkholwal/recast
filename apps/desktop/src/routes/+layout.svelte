<script lang="ts">
  import "@fontsource-variable/google-sans";
  import { TooltipProvider } from "@recast/ui/tooltip";
  import "../app.css";

  let { children } = $props();

  import Loading from "$components/layout/loading.svelte";
  import { initAssets } from "$lib/assets";
  import { getTauriTheme, isTauriApp } from "$lib/runtime/tauri";
  import { Toaster } from "@recast/ui/sonner";
  import { ModeWatcher, setMode } from "@recast/ui/theme";
  import { onMount, tick } from "svelte";

  // Kick off external-asset download (wallpapers etc.) on first paint. Safe in
  // both browser and Tauri runtimes — no-op in the browser.
  initAssets();

  // Remove the boot splash screen after the app is mounted
  onMount(async () => {
    await tick();
    const boot = document.getElementById("boot");
    if (boot) {
      boot.classList.add("boot-leaving");
      setTimeout(() => boot.remove(), 280);
    }

    if (await isTauriApp()) {
      const theme = await getTauriTheme();
      const stored = localStorage.getItem("mode-watcher-mode");
      if (theme && (!stored || stored === "system")) {
        setMode(theme);
      }
    }
  });
</script>
<TooltipProvider>
  <Loading />
  <ModeWatcher />
  <Toaster position="top-center" richColors />
  <div class="relative flex min-h-screen w-full flex-col bg-background">
    {@render children()}
  </div>
</TooltipProvider>
