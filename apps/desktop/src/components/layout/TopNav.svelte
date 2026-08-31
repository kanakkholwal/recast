<script lang="ts">
import {
	CircleFilled,
	Moon,
	Search,
	Settings,
	Share2,
	SlidersHorizontal,
	Sun,
	Video,
	Wand2,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { NotchedShelf } from "@recast/ui/notched-shelf";
import { mode, toggleModeCircleBlur } from "@recast/ui/theme";
import { cn } from "@recast/ui/utils";
import { page } from "$app/state";
import SidebarAccount from "$components/layout/SidebarAccount.svelte";
import Logo from "$components/logo.svelte";
import { launchRecordingPanel } from "$lib/ipc";
import { chordLabel } from "$lib/shortcuts/registry.svelte";
import { commandPalette } from "$lib/stores/command-palette.svelte";

const path = $derived(page.url.pathname);
const recordShortcut = $derived(chordLabel("general.record"));
const paletteShortcut = $derived(chordLabel("general.palette"));

const tabs = [
	{ label: "Record", href: "/", icon: Video },
	{ label: "Polish", href: "/recasts", icon: Wand2 },
	{ label: "Share", href: "/exports", icon: Share2 },
];

const isActive = (href: string) => (href === "/" ? path === "/" : path.startsWith(href));
</script>

<div class="relative z-30 shrink-0 bg-card" data-recast-topnav data-tauri-drag-region>
  <div class="flex h-14 items-center gap-2 px-3">
    <a href="/" class="flex shrink-0 items-center gap-2 rounded-lg px-1 hover:opacity-80" aria-label="Recast home">
      <Logo size="22" class="shrink-0" />
      <span class="text-[14px] font-semibold tracking-tight text-foreground">Recast</span>
    </a>

    <div class="h-full flex-1" data-tauri-drag-region></div>

    <button
      type="button"
      onclick={() => commandPalette.show()}
      aria-label="Search"
      title={`Search · ${paletteShortcut}`}
      class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-foreground/5 hover:text-foreground"
    >
      <Search size={16} />
    </button>

    <Button
      size="sm"
      class="gap-1.5 bg-foreground text-background hover:bg-foreground/90"
      onclick={() => launchRecordingPanel()}
      title={`Launch recording panel · ${recordShortcut}`}
    >
      <CircleFilled class="size-3" /> Launch Panel
    </Button>

    <a
      href="/profiles"
      aria-label="Capture profiles"
      title="Capture profiles"
      class={cn(
        "inline-flex size-8 items-center justify-center rounded-md transition-colors duration-150",
        isActive("/profiles") ? "bg-foreground/10 text-foreground" : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground",
      )}
    >
      <SlidersHorizontal size={16} />
    </a>
    <a
      href="/settings"
      aria-label="Settings"
      title="Settings"
      class={cn(
        "inline-flex size-8 items-center justify-center rounded-md transition-colors duration-150",
        isActive("/settings") ? "bg-foreground/10 text-foreground" : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground",
      )}
    >
      <Settings size={16} />
    </a>
    <button
      type="button"
      onclick={(e) => toggleModeCircleBlur({ x: e.clientX, y: e.clientY })}
      aria-label="Toggle theme"
      title="Toggle light / dark"
      class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-foreground/5 hover:text-foreground motion-safe:active:scale-95"
    >
      {#if mode.current === "dark"}<Sun size={16} />{:else}<Moon size={16} />{/if}
    </button>
    <div class="ml-0.5"><SidebarAccount open={false} /></div>
  </div>

  <!-- Record / Polish / Share hang from the bar's bottom edge into the page. -->
  <div class="pointer-events-none absolute inset-x-0 bottom-0 z-50 flex translate-y-full justify-center">
    <NotchedShelf fill="text-card" class="pointer-events-auto h-12 w-fit!">
      <div class="flex items-center gap-0.5" style="view-transition-name: recast-nav-tabs">
        {#each tabs as tab (tab.href)}
          {@const on = isActive(tab.href)}
          {@const Icon = tab.icon}
          <a
            href={tab.href}
            aria-current={on ? "page" : undefined}
            class={cn(
              "relative flex items-center gap-1.5 rounded-lg px-3.5 py-1.5 text-[12.5px] font-semibold tracking-tight transition-colors duration-200",
              on ? "text-background" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {#if on}
              <span
                aria-hidden="true"
                style="view-transition-name: recast-nav-pill"
                class="absolute inset-0 -z-10 rounded-lg bg-foreground shadow-craft-md"
              ></span>
            {/if}
            <Icon size={14} />
            {tab.label}
          </a>
        {/each}
      </div>
    </NotchedShelf>
  </div>
</div>
