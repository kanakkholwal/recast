<script lang="ts">
  import { page } from "$app/state";
  import SearchCommandMenu from "$components/layout/SearchCommandMenu.svelte";
  import Logo from "$components/logo.svelte";
  import { launchRecordingPanel } from "$lib/ipc";
  import {
    Download,
    Film,
    LayoutDashboard,
    Radio,
    Settings,
    Sparkles,
    SlidersHorizontal,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Sidebar from "@recast/ui/sidebar";
  import { useSidebar } from "@recast/ui/sidebar";
  import { cn } from "@recast/ui/utils";
  import type { ComponentProps } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { crossfade, fade, fly } from "svelte/transition";

  let {
    ref = $bindable(null),
    ...restProps
  }: ComponentProps<typeof Sidebar.Root> = $props();

  // Read the parent <Sidebar.Provider> state so transitions can fire on
  // open/collapse rather than being purely CSS-driven.
  const sidebar = useSidebar();
  const open = $derived(sidebar.state === "expanded");

  let currentPath = $derived(page.url.pathname);
  function isActive(path: string) {
    if (path === "/") return currentPath === "/";
    return currentPath.startsWith(path);
  }

  const navLinks = [
    { title: "Home", href: "/", icon: LayoutDashboard },
    { title: "Recasts", href: "/recasts", icon: Film },
    { title: "Exports", href: "/exports", icon: Download },
    { title: "Profiles", href: "/profiles", icon: SlidersHorizontal },
    { title: "What's new", href: "/whats-new", icon: Sparkles },
    { title: "Settings", href: "/settings", icon: Settings },
  ];

  // Crossfade between active rows so the highlight slides between items.
  const [send, receive] = crossfade({
    duration: 280,
    easing: cubicOut,
    fallback: (node) => fade(node, { duration: 120 }),
  });
</script>

<Sidebar.Root bind:ref variant="floating" collapsible="icon" {...restProps}>
  <Sidebar.Rail class="data-[state=collapsed]:hidden" />

  <Sidebar.Header class="gap-3 py-3">
    <Sidebar.MenuItem class="relative">
      <a
        href="/"
        class={cn(
          "flex h-10 items-center gap-2.5 overflow-hidden rounded-lg transition-opacity hover:opacity-80",
          open ? "px-2 pr-9" : "justify-center px-0",
        )}
        data-tauri-drag-region
        aria-label="Recast — home"
      >
        <Logo
          size="24"
          // color="var(--foreground)"
          // fill="var(--background)"
          class="shrink-0"
          data-tauri-drag-region
        />
        {#if open}
          <span
            in:fly={{ x: -8, duration: 240, easing: cubicOut, delay: 60 }}
            out:fade={{ duration: 220, easing: cubicOut }}
            class="truncate text-[15px] font-semibold tracking-tight text-foreground"
            data-tauri-drag-region
          >
            Recast
          </span>
        {/if}
      </a>

    </Sidebar.MenuItem>

    <Sidebar.MenuItem>
      <SearchCommandMenu iconOnly={!open} />
    </Sidebar.MenuItem>
  </Sidebar.Header>

  <Sidebar.Content class="scrollbar-hide">
    <Sidebar.Group>
      {#if open}
        <Sidebar.GroupLabel
          class="px-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >
          <span
            in:fade={{ duration: 180, delay: 80, easing: cubicOut }}
            out:fade={{ duration: 140, easing: cubicOut }}
          >
            Workspace
          </span>
        </Sidebar.GroupLabel>
      {/if}
      <Sidebar.GroupContent>
        <Sidebar.Menu class="gap-0.5">
          {#each navLinks as link (link.href)}
            {@const active = isActive(link.href)}
            {@const Icon = link.icon}
            <Sidebar.MenuItem>
              <Sidebar.MenuButton tooltipContent={link.title}>
                {#snippet child({
                  props,
                }: {
                  props: ComponentProps<typeof Sidebar.MenuButton>;
                })}
                  <a
                    href={link.href}
                    {...(props as Record<string, unknown>)}
                    data-active={active}
                    class={cn(
                      "group/item relative flex h-9 items-center gap-2.5 overflow-hidden rounded-lg text-[12.5px] font-medium transition-colors duration-200",
                      active
                        ? "text-foreground"
                        : "text-muted-foreground hover:text-foreground",
                      open ? "px-2.5" : "size-8 justify-center p-0",
                    )}
                  >
                    {#if active}
                      <span
                        in:receive={{ key: "sidebar-active" }}
                        out:send={{ key: "sidebar-active" }}
                        class="absolute inset-0 z-0 rounded-lg bg-foreground/6 ring-1 ring-inset ring-border/40 shadow-(--shadow-craft-inset)"
                        aria-hidden="true"
                      ></span>
                      {#if open}
                        <span
                          in:receive={{ key: "sidebar-active-pill" }}
                          out:send={{ key: "sidebar-active-pill" }}
                          class="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-primary"
                          aria-hidden="true"
                        ></span>
                      {/if}
                    {/if}
                    <Icon
                      size={14}
                      class={cn(
                        "relative z-10 shrink-0 transition-transform duration-200",
                        "group-hover/item:-translate-y-px group-active/item:scale-95",
                      )}
                    />
                    {#if open}
                      <span
                        in:fly={{ x: -6, duration: 220, easing: cubicOut, delay: 40 }}
                        out:fade={{ duration: 160, easing: cubicOut }}
                        class="relative z-10 truncate"
                      >
                        {link.title}
                      </span>
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
      class={cn(
        "group/launch w-full gap-1.5 overflow-hidden rounded-lg",
        open ? "h-9" : "size-8 p-0",
      )}
      title="Launch Recording Panel (⌘⇧R)"
    >
      <Radio
        size={13}
        class="shrink-0 transition-transform duration-200 group-hover/launch:rotate-12"
      />
      {#if open}
        <span
          in:fly={{ x: -6, duration: 220, easing: cubicOut, delay: 60 }}
          out:fade={{ duration: 160, easing: cubicOut }}
          class="text-[12px] font-semibold"
        >
          Launch Panel
        </span>
      {/if}
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
