<script lang="ts">
  import "@fontsource-variable/google-sans";
  import { TooltipProvider } from "@recast/ui/tooltip";
  import "../app.css";

  import { onNavigate } from "$app/navigation";
  import { page } from "$app/state";

  let { children } = $props();

  import CommandPaletteHost from "$components/layout/CommandPaletteHost.svelte";
  import { initAssets } from "$lib/assets";
  import { NavProgress } from "@recast/ui/nav-progress";
  import { getTauriTheme, isTauriApp } from "$lib/runtime/tauri";
  import { Toaster } from "@recast/ui/sonner";
  import { ModeWatcher, setMode } from "@recast/ui/theme";
  import { onMount, tick } from "svelte";

  const TRANSPARENT_ROUTES = [
    "/camera-preview",
    "/device-picker",
    "/profile-picker",
    "/select",
    "/panel",
  ];
  const isTransparentRoute = $derived(
    TRANSPARENT_ROUTES.some((p) => page.url.pathname.startsWith(p)),
  );

  // Native macOS-style page transitions via the View Transitions API.
  // Skipped for overlay/secondary windows (transparent routes) and when the
  // user prefers reduced motion — CSS handles the reduced-motion case too.
  onNavigate((navigation) => {
    if (typeof document === "undefined") return;
    if (!("startViewTransition" in document)) return;

    const to = navigation.to?.url.pathname ?? "";
    const from = navigation.from?.url.pathname ?? "";
    const isOverlay = (p: string) =>
      TRANSPARENT_ROUTES.some((r) => p.startsWith(r));
    if (isOverlay(to) || isOverlay(from)) return;

    document.documentElement.dataset.navDirection =
      to.length >= from.length ? "forward" : "back";

    return new Promise((resolve) => {
      document.startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });
    });
  });

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
  <NavProgress />
  <ModeWatcher />
  <!-- Overlay windows (panel, camera-preview, pickers) are too small to host
       a Sonner toast without overflow. Gate the Toaster out of those routes so
       downstream code that calls `toast.*` is just a no-op there — the main
       window keeps its toaster as usual. -->
  {#if !isTransparentRoute}
    <!-- Position/styling defaults live in @recast/ui/sonner so every
         consumer (desktop, web) gets the same bottom-right glass-card
         notification language matching the auto-updater stack. Override
         here only if a specific route needs a different placement. -->
    <Toaster />
    <!-- Command palette host: owns the ⌘K shortcut + dialog so they work on
         every route (editor included), not just the (app) sidebar layout. -->
    <CommandPaletteHost />
  {/if}
  <div
    class="relative flex min-h-screen min-w-dvw w-full flex-col {isTransparentRoute
      ? 'bg-transparent'
      : 'bg-background'}"
  >
    {@render children()}
  </div>
</TooltipProvider>
