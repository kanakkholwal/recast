<script lang="ts">
import { page } from "$app/state";
import SearchCommandMenu from "$components/layout/SearchCommandMenu.svelte";
import SidebarAccount from "$components/layout/SidebarAccount.svelte";
import Logo from "$components/logo.svelte";
import { launchRecordingPanel } from "$lib/ipc";
import { chordLabel, shortcutsDialog } from "$lib/shortcuts/registry.svelte";
import {
	Download,
	LayoutDashboard,
	Broadcast,
	Keyboard,
	Moon,
	Settings,
	SlidersHorizontal,
	Sun,
	Video,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { mode, toggleMode } from "@recast/ui/theme";
import * as Sidebar from "@recast/ui/sidebar";
import { useSidebar } from "@recast/ui/sidebar";
import { cn } from "@recast/ui/utils";
import type { ComponentProps } from "svelte";
import { cubicOut } from "svelte/easing";
import { crossfade, fade } from "svelte/transition";
import { isActive } from "./app-sidebar.logic";

let {
	ref = $bindable(null),
	variant = "inset",
	...restProps
}: ComponentProps<typeof Sidebar.Root> = $props();

// Read the parent <Sidebar.Provider> state so transitions can fire on
// open/collapse rather than being purely CSS-driven.
const sidebar = useSidebar();
const open = $derived(sidebar.state === "expanded");

let currentPath = $derived(page.url.pathname);

// From the registry, so the tooltip can't drift from the real binding. The
// hardcoded one here read "⌘⇧R", a chord that launches nothing.
const recordShortcut = $derived(chordLabel("general.record"));

// Split destinations (things you make/browse) from configuration so Settings
// and Profiles read as a distinct band rather than trailing the content nav.
const navGroups = [
	{
		label: "Workspace",
		links: [
			{ title: "Home", href: "/", icon: LayoutDashboard },
			{ title: "Recordings", href: "/recasts", icon: Video },
			{ title: "Exports", href: "/exports", icon: Download },
		],
	},
	{
		label: "Configure",
		links: [
			{ title: "Profiles", href: "/profiles", icon: SlidersHorizontal },
			{ title: "Settings", href: "/settings", icon: Settings },
		],
	},
];

// Crossfade between active rows so the highlight slides between items.
const [send, receive] = crossfade({
	duration: 280,
	easing: cubicOut,
	fallback: (node) => fade(node, { duration: 120 }),
});
</script>

<Sidebar.Root bind:ref {variant} collapsible="icon" {...restProps}>
  <Sidebar.Rail class="data-[state=collapsed]:hidden" />

  <!-- Drag region on the header, not on the logo link: Tauri starts a window
       drag from the element carrying the attribute, and on an <a> that competes
       with the click. Children without it still behave normally. -->
  <Sidebar.Header class="gap-3 py-3" data-tauri-drag-region>
    <Sidebar.MenuItem class="relative">
      <a
        href="/"
        class={cn(
          "flex h-10 items-center gap-2.5 overflow-hidden rounded-lg transition-[padding,opacity] duration-200 ease-linear hover:opacity-80",
          open ? "px-2 pr-9" : "px-1.5",
        )}
        aria-label="Recast home"
      >
        <Logo size="24" class="shrink-0" />
        <!-- Always mounted: the label collapses its own width in sync with the
             sidebar-container width animation, so nothing snaps on toggle. -->
        <span
          class={cn(
            "truncate text-[15px] font-semibold tracking-tight text-foreground transition-[max-width,opacity] duration-200 ease-linear",
            open ? "max-w-32 opacity-100" : "max-w-0 opacity-0",
          )}
        >
          Recast
        </span>
      </a>
    </Sidebar.MenuItem>

    <Sidebar.MenuItem>
      <SearchCommandMenu iconOnly={!open} />
    </Sidebar.MenuItem>
  </Sidebar.Header>

  <Sidebar.Content class="no-scrollbar">
    {#each navGroups as group (group.label)}
    <Sidebar.Group>
      <!-- Kept mounted: GroupLabel has a built-in collapse
           (`group-data-[collapsible=icon]:-mt-8 opacity-0`, transitioned), so
           it slides away smoothly instead of popping out of the DOM. -->
      <Sidebar.GroupLabel
        class="px-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        {group.label}
      </Sidebar.GroupLabel>
      <Sidebar.GroupContent>
        <Sidebar.Menu class="gap-0.5">
          {#each group.links as link (link.href)}
            {@const active = isActive(link.href, currentPath)}
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
                    aria-current={active ? "page" : undefined}
                    class={cn(
                      "group/item relative flex h-9 w-full items-center gap-2.5 overflow-hidden rounded-lg px-2.5 text-[12.5px] font-medium transition-colors duration-200",
                      active
                        ? "text-foreground"
                        : "text-muted-foreground hover:text-foreground",
                    )}
                  >
                    {#if active}
                      <span
                        in:receive={{ key: "sidebar-active" }}
                        out:send={{ key: "sidebar-active" }}
                        class="absolute inset-0 z-0 rounded-lg bg-primary/10 ring-1 ring-inset ring-primary/25"
                        aria-hidden="true"
                      ></span>
                    {/if}
                    <Icon
                      size={14}
                      class={cn(
                        "relative z-10 shrink-0 transition-[transform,color] duration-200",
                        "group-hover/item:-translate-y-px group-active/item:scale-95",
                        active && "text-primary",
                      )}
                    />
                    <span
                      class={cn(
                        "relative z-10 truncate transition-[max-width,opacity] duration-200 ease-linear",
                        open ? "max-w-40 opacity-100" : "max-w-0 opacity-0",
                      )}
                    >
                      {link.title}
                    </span>
                  </a>
                {/snippet}
              </Sidebar.MenuButton>
            </Sidebar.MenuItem>
          {/each}
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
    {/each}
  </Sidebar.Content>

  <Sidebar.Footer class="flex flex-col gap-2 border-t border-border/30 p-2">
    <Button
      onclick={() => launchRecordingPanel()}
      size="sm"
      class={cn(
        "group/launch h-9 w-full justify-center overflow-hidden rounded-lg transition-[padding] duration-200 ease-linear",
        open ? "px-3 gap-1.5" : "px-0 gap-0",
      )}
      title={`Launch recording panel  ·  ${recordShortcut}`}
    >
      <Broadcast
        size={13}
        class="shrink-0 transition-transform duration-200 group-hover/launch:rotate-12"
      />
      <span
        class={cn(
          "overflow-hidden text-[12px] font-semibold transition-[max-width,opacity] duration-200 ease-linear",
          open ? "max-w-32 opacity-100" : "max-w-0 opacity-0",
        )}
      >
        Launch Panel
      </span>
    </Button>

    <SidebarAccount {open} />

    <div
      class={cn(
        "flex border-t border-border/30 pt-2",
        open ? "items-center justify-center gap-1" : "flex-col items-center gap-1",
      )}
    >
      <button
        type="button"
        onclick={toggleMode}
        aria-label="Toggle theme"
        title="Toggle light / dark"
        class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-foreground/5 hover:text-foreground motion-safe:active:scale-95"
      >
        {#if mode.current === "dark"}
          <Sun size={15} />
        {:else}
          <Moon size={15} />
        {/if}
      </button>
      <button
        type="button"
        onclick={() => shortcutsDialog.show()}
        aria-label="Keyboard shortcuts"
        title="Keyboard shortcuts"
        class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-foreground/5 hover:text-foreground motion-safe:active:scale-95"
      >
        <Keyboard size={15} />
      </button>
    </div>
  </Sidebar.Footer>
</Sidebar.Root>

