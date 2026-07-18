<script lang="ts">
  import { buildGlobalCommands } from "$lib/commands";
  import {
    commandPalette,
    type PaletteCommand,
  } from "$lib/stores/command-palette.svelte";
  import {
    groupCommands,
    highlight,
    rankCommands,
  } from "./command-palette-host.logic";
  import { CornerDownLeft, Search } from "@recast/icons";
  import { Kbd, KbdGroup } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";
  import { onMount, tick } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, scale } from "svelte/transition";

  let query = $state("");
  let selectedIndex = $state(0);
  let inputRef = $state<HTMLInputElement | null>(null);
  let listRef = $state<HTMLDivElement | null>(null);
  let contentHeight = $state(0);

  // Safe on every mount: registerMany dedupes by id. The Mod+K open shortcut
  // lives in the central registry (general.palette), not a window listener here.
  onMount(() => {
    commandPalette.registerMany(buildGlobalCommands());
  });

  function close() {
    commandPalette.hide();
  }

  function runCommand(command: PaletteCommand) {
    close();
    queueMicrotask(() => command.action());
  }

  const filtered = $derived(rankCommands(commandPalette.commands, query));

  const grouped = $derived.by<[string, PaletteCommand[]][]>(() =>
    groupCommands(filtered, query),
  );

  // Flat order used for keyboard navigation; mirrors render order so the
  // selected index always points at a real button.
  const flatItems = $derived(grouped.flatMap(([, cmds]) => cmds));

  $effect(() => {
    void filtered;
    selectedIndex = 0;
  });

  $effect(() => {
    if (commandPalette.open) {
      // Reset query each open; focus after the dialog has mounted.
      query = "";
      tick().then(() => inputRef?.focus());
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!commandPalette.open) return;

    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (flatItems.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % flatItems.length;
      scrollSelectedIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex =
        (selectedIndex - 1 + flatItems.length) % flatItems.length;
      scrollSelectedIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cmd = flatItems[selectedIndex];
      if (cmd) runCommand(cmd);
    }
  }

  function scrollSelectedIntoView() {
    if (!listRef) return;
    const el = listRef.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }

  $effect(() => {
    if (commandPalette.open) {
      window.addEventListener("keydown", handleKeydown);
      return () => window.removeEventListener("keydown", handleKeydown);
    }
  });

  function indexOfCmd(cmd: PaletteCommand): number {
    return flatItems.indexOf(cmd);
  }
</script>

{#if commandPalette.open}
  <div
    class="fixed inset-0 z-60 bg-background/70 backdrop-blur-sm"
    transition:fade={{ duration: 150 }}
    onclick={close}
    role="presentation"
  ></div>

  <div
    class="fixed inset-0 z-60 flex items-start justify-center p-4 sm:pt-[12vh]"
    role="dialog"
    aria-modal="true"
    aria-label="Command palette"
    tabindex="-1"
    onclick={(e) => e.target === e.currentTarget && close()}
    onkeydown={(e) => e.key === "Escape" && close()}
  >
    <div
      class="relative w-full max-w-xl transform-gpu overflow-hidden rounded-xl border border-border bg-popover shadow-2xl ring-1 ring-black/5"
      role="document"
      transition:scale={{ duration: 220, start: 0.96, easing: cubicOut }}
    >
      <div class="flex items-center gap-2 border-b border-border/60 px-3">
        <Search class="size-4 shrink-0 text-muted-foreground/70" />
        <input
          bind:this={inputRef}
          bind:value={query}
          class="command-palette-input flex h-12 w-full bg-transparent text-sm tracking-normal text-foreground placeholder:text-muted-foreground/70 focus:outline-none"
          placeholder="Search commands…"
          aria-label="Search commands"
        />
        <Kbd class="hidden sm:inline-flex">Esc</Kbd>
      </div>

      <div
        class="overflow-hidden transition-[height] duration-300 ease-out"
        style="height: {contentHeight}px"
      >
        <div bind:clientHeight={contentHeight}>
          {#if flatItems.length > 0}
            <div
              bind:this={listRef}
              class="scrollbar-transparent max-h-96 overflow-y-auto p-2"
              style="mask-image: linear-gradient(to bottom, transparent, black 8px, black calc(100% - 8px), transparent); -webkit-mask-image: linear-gradient(to bottom, transparent, black 8px, black calc(100% - 8px), transparent);"
            >
              {#each grouped as [category, cmds] (category)}
                {#if !query.trim()}
                  <div
                    class="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70"
                  >
                    {category}
                  </div>
                {/if}
                {#each cmds as cmd (cmd.id)}
                  {@const i = indexOfCmd(cmd)}
                  {@const Icon = cmd.icon}
                  {@const active = i === selectedIndex}
                  <button
                    type="button"
                    data-index={i}
                    class={cn(
                      "group relative flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors",
                      active
                        ? "bg-muted text-foreground"
                        : "text-foreground/90 hover:bg-muted/60",
                    )}
                    onclick={() => runCommand(cmd)}
                    onmouseenter={() => (selectedIndex = i)}
                  >
                    {#if Icon}
                      <span
                        class={cn(
                          "flex size-5 shrink-0 items-center justify-center transition-colors",
                          active
                            ? "text-foreground"
                            : "text-muted-foreground",
                        )}
                      >
                        <Icon size={14} />
                      </span>
                    {/if}
                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span class="truncate text-xs font-medium">
                        {#each highlight(cmd.title, query) as part, k (k)}
                          {#if part.hl}
                            <span class="text-primary">{part.text}</span>
                          {:else}{part.text}{/if}
                        {/each}
                      </span>
                      {#if cmd.description}
                        <span
                          class="truncate text-[10px] font-medium text-muted-foreground"
                        >
                          {#each highlight(cmd.description, query) as part, k (k)}
                            {#if part.hl}
                              <span class="text-primary">{part.text}</span>
                            {:else}{part.text}{/if}
                          {/each}
                        </span>
                      {/if}
                    </div>
                    {#if cmd.shortcut}
                      <Kbd class="ml-auto hidden sm:inline-flex">
                        {cmd.shortcut}
                      </Kbd>
                    {/if}
                  </button>
                {/each}
              {/each}
            </div>
          {:else if query}
            <div class="px-4 py-10 text-center">
              <p class="text-sm font-medium text-foreground">No results</p>
              <p class="mt-1 text-xs text-muted-foreground">
                Try a different search term
              </p>
            </div>
          {/if}
        </div>
      </div>

      <div
        class="flex items-center justify-between gap-3 border-t border-border/60 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground"
      >
        <span class="flex items-center gap-1.5">
          <Kbd>
            <CornerDownLeft class="size-3" />
          </Kbd>
          <span>Run</span>
        </span>
        <span class="flex items-center gap-3">
          <span class="hidden items-center gap-1.5 sm:flex">
            <KbdGroup>
              <Kbd>↑</Kbd>
              <Kbd>↓</Kbd>
            </KbdGroup>
            <span>Navigate</span>
          </span>
          <span class="font-medium">Recast</span>
        </span>
      </div>
    </div>
  </div>
{/if}

<style>
  .command-palette-input:focus,
  .command-palette-input:focus-visible {
    outline: none !important;
    outline-color: transparent !important;
    outline-offset: 0 !important;
    box-shadow: none !important;
  }
</style>
