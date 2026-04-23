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
  <Sidebar.Header class="gap-3 py-3">
    <Sidebar.MenuItem>
      <a
        href="/"
        class="group flex items-center gap-2.5 transition-opacity hover:opacity-80"
        data-tauri-drag-region
      >
        <div
          class="flex size-7 shrink-0 items-center justify-center rounded-lg text-primary-foreground"
          data-tauri-drag-region
        >
          <Logo size="18" color="var(--primary)" />
        </div>
        <h1
          class="text-[13px] font-semibold tracking-tight text-foreground group-data-[state=collapsed]:hidden"
          data-tauri-drag-region
        >
          Recast
        </h1>
      </a>
    </Sidebar.MenuItem>

    <Sidebar.MenuItem>
      <SearchCommandMenu />
    </Sidebar.MenuItem>
  </Sidebar.Header>

  <Sidebar.Content class="scrollbar-hide">
    <Sidebar.Group>
      <Sidebar.GroupLabel class="px-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground group-data-[state=collapsed]:hidden">
        Workspace
      </Sidebar.GroupLabel>
      <Sidebar.GroupContent>
        <Sidebar.Menu class="gap-0.5">
          {#each navLinks as navLink (navLink.href)}
            {@const active = isActive(navLink.href)}
            {@const Icon = navLink.icon}
            <Sidebar.MenuItem>
              <Sidebar.MenuButton tooltipContent={navLink.title}>
                {#snippet child({ props })}
                  <a
                    href={navLink.href}
                    {...props}
                    data-active={active}
                    class={cn(
                      "group relative flex h-8 items-center gap-2.5 rounded-lg px-2.5 text-[12.5px] font-medium transition-colors",
                      active
                        ? "bg-card/80 text-foreground ring-1 ring-inset ring-border/50 shadow-(--shadow-craft-inset)"
                        : "text-muted-foreground hover:bg-foreground/4 hover:text-foreground",
                      "group-data-[state=collapsed]:size-8 group-data-[state=collapsed]:justify-center group-data-[state=collapsed]:p-0",
                    )}
                  >
                    <Icon size={14} class="shrink-0" />
                    <span class="group-data-[state=collapsed]:hidden">{navLink.title}</span>
                    {#if active}
                      <span
                        class="absolute top-1/2 left-0 h-3 w-0.5 -translate-x-1.5 -translate-y-1/2 rounded-full bg-primary group-data-[state=collapsed]:hidden"
                        aria-hidden="true"
                      ></span>
                    {/if}
                  </a>
                {/snippet}
              </Sidebar.MenuButton>
            </Sidebar.MenuItem>
          {/each}
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
  </Sidebar.Content>

  <Sidebar.Footer class="border-t border-border/30 p-2">
    <Button
      onclick={launchRecordingPanel}
      size="sm"
      class="group h-9 w-full gap-1.5 rounded-lg group-data-[state=collapsed]:size-8 group-data-[state=collapsed]:p-0"
      title="Launch Recording Panel (⌘⇧R)"
    >
      <Radio size={13} class="shrink-0" />
      <span class="text-[12px] font-semibold group-data-[state=collapsed]:hidden">
        Launch Panel
      </span>
    </Button>
  </Sidebar.Footer>
</Sidebar.Root>

<style>
  .scrollbar-hide {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
  .scrollbar-hide::-webkit-scrollbar {
    display: none;
  }
</style>
