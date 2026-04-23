<script lang="ts">
  import { page } from "$app/state";
  import SearchCommandMenu from "$components/layout/SearchCommandMenu.svelte";
  import Logo from "$components/logo.svelte";
  import { launchRecordingPanel } from "$lib/ipc";
  import { Download, Film, LayoutDashboard, Radio, Settings, SlidersHorizontal } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Sidebar from "@recast/ui/sidebar";
  import { cn } from "@recast/ui/utils";
  import type { ComponentProps } from "svelte";

  let currentPath = $derived(page.url.pathname);
  const navLinks = [
    { title: "Home", href: "/", icon: LayoutDashboard },
    { title: "Recasts", href: "/recasts", icon: Film },
    { title: "Exports", href: "/exports", icon: Download },
    { title: "Profiles", href: "/profiles", icon: SlidersHorizontal },
    { title: "Settings", href: "/settings", icon: Settings },
  ];

  function isActive(path: string) {
    if (path === "/") return currentPath === "/";
    return currentPath.startsWith(path);
  }

  let { ref = $bindable(null), ...restProps }: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root bind:ref variant="sidebar" collapsible="icon" {...restProps}>
  <Sidebar.Rail class="data-[state=collapsed]:hidden" />
  <Sidebar.Header class="px-2 py-2">
    <Sidebar.MenuItem>
      <a
        href="/"
        class="group flex items-center gap-2.5 transition-opacity hover:opacity-80"
      >
        <div
          class="flex size-7 shrink-0 items-center justify-center rounded-lg text-primary-foreground"
          data-tauri-drag-region
        >
          <!-- <Hexagon size={15} class="fill-current" strokeWidth={2.5} /> -->
           <Logo size="18" color="var(--primary)" />
        </div>
        <h1
          class="text-[13px] font-semibold tracking-tight group-data-[state=collapsed]:hidden"
          data-tauri-drag-region
        >
          Recast
        </h1>
      </a>
    </Sidebar.MenuItem>

    <Sidebar.MenuItem class="mt-1">
      <SearchCommandMenu  />
    </Sidebar.MenuItem>
  </Sidebar.Header>

  <Sidebar.Content class="scrollbar-hide">
    <Sidebar.Group>
      <Sidebar.GroupLabel class="text-[10px] font-semibold uppercase tracking-wider"
        >Workspace</Sidebar.GroupLabel
      >
      <Sidebar.GroupContent>
        <Sidebar.Menu class="gap-2">
          {#each navLinks as navLink (navLink.href)}
            <Sidebar.MenuItem>
              <Sidebar.MenuButton tooltipContent={navLink.title}>
                {#snippet child({ props })}
                  <a
                    href={navLink.href}
                    {...props}
                    class={cn(
                      "group flex items-center gap-2.5 rounded-md p-2 px-3 text-sm font-medium transition-colors",
                      isActive(navLink.href)
                        ? "bg-muted text-foreground"
                        : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
                      "group-data-[state=collapsed]:size-8 group-data-[state=collapsed]:justify-center group-data-[state=collapsed]:p-0",
                    )}
                  >
                    {#if navLink.icon}
                      {@const Icon = navLink.icon}
                      <Icon size={14} class="shrink-0" />
                    {/if}
                    <span class="group-data-[state=collapsed]:hidden">{navLink.title}</span>
                  </a>
                {/snippet}
              </Sidebar.MenuButton>
            </Sidebar.MenuItem>
          {/each}
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
  </Sidebar.Content>

  <Sidebar.Footer class="border-t border-border/40 p-2">
    <Button
      onclick={launchRecordingPanel}
      size="sm"
      class="group relative h-8 w-full gap-1.5 group-data-[state=collapsed]:size-8 group-data-[state=collapsed]:p-0"
      title="Launch Recording Panel"
    >
      <Radio size={14} class="animate-pulse shrink-0" />
      <span class="group-data-[state=collapsed]:hidden">Launch Panel</span>
    </Button>
  </Sidebar.Footer>
</Sidebar.Root>

<style>
  /* Hide scrollbar for cleaner look */
  .scrollbar-hide {
    -ms-overflow-style: none; /* IE and Edge */
    scrollbar-width: none; /* Firefox */
  }
  .scrollbar-hide::-webkit-scrollbar {
    display: none;
  }
</style>
