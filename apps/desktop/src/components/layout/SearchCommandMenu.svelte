<script lang="ts">
  import { buildGlobalCommands } from "$lib/commands";
  import { commandPalette, type PaletteCommand } from "$lib/stores/command-palette.svelte";
  import { Search } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Command from "@recast/ui/command";
  import * as Dialog from "@recast/ui/dialog";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";

  let { iconOnly } = $props<{ iconOnly?: boolean }>();

  // Register global commands once on mount
  onMount(() => {
    commandPalette.registerMany(buildGlobalCommands());
    window.addEventListener("keydown", handleGlobalKeydown);
    return () => window.removeEventListener("keydown", handleGlobalKeydown);
  });

  function handleGlobalKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      commandPalette.toggle();
    }
  }

  function runCommand(command: PaletteCommand) {
    commandPalette.hide();
    queueMicrotask(() => command.action());
  }

  const grouped = $derived.by(() => {
    const map = new Map<string, PaletteCommand[]>();
    for (const cmd of commandPalette.commands) {
      if (!map.has(cmd.category)) map.set(cmd.category, []);
      map.get(cmd.category)!.push(cmd);
    }
    return Array.from(map.entries());
  });
</script>

<Button
  onclick={() => commandPalette.show()}
  aria-label="Open Command Menu"
  title="Open Command Menu (⌘K)"
  variant="raw"
  size="sm"
  class={cn(
    "border border-border group relative h-8 bg-card",
    iconOnly ? "w-8" : "min-w-8 w-full max-w-xs",
  )}
>
  <Search class="size-4 shrink-0 opacity-50 transition-opacity group-hover:opacity-70" />
  {#if !iconOnly}
    <span class="flex-1 text-left text-xs font-medium group-data-[state=collapsed]:hidden!">
      Search...
    </span>
    <kbd
      class="group-data-[state=collapsed]:hidden! hidden items-center gap-1 rounded-md border border-border/40 bg-background/50 px-2 py-1 font-mono text-[11px] font-medium text-muted-foreground/70 backdrop-blur-sm sm:inline-flex"
    >
      <span class="text-xs font-semibold group-data-[state=collapsed]:hidden">⌘</span>K
    </kbd>
  {/if}
</Button>

<Dialog.Root bind:open={() => commandPalette.open, (v) => (commandPalette.open = v)}>
  <Dialog.Content
    showCloseButton={false}
    class="top-[20%] max-w-xl translate-y-0 overflow-hidden rounded-xl p-0 ring-1 ring-border"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>Command Palette</Dialog.Title>
      <Dialog.Description>Search across the application</Dialog.Description>
    </Dialog.Header>
    <Command.Root class="rounded-xl bg-popover">
      <Command.Input placeholder="Search anything..." />
      <Command.List class="max-h-96 scrollbar-transparent">
        <Command.Empty>
          <div class="py-8 text-center">
            <p class="text-sm font-medium text-foreground">No results</p>
            <p class="mt-1 text-xs text-muted-foreground">Try a different search term</p>
          </div>
        </Command.Empty>
        {#each grouped as [category, cmds] (category)}
          <Command.Group heading={category}>
            {#each cmds as cmd (cmd.id)}
              {@const Icon = cmd.icon}
              <Command.Item
                value={cmd.id + " " + cmd.title}
                keywords={[cmd.title, cmd.description ?? "", ...(cmd.keywords ?? [])]}
                onSelect={() => runCommand(cmd)}
                class="h-9 gap-3 rounded-md px-2 cursor-pointer"
              >
                {#if Icon}
                  <span class="flex size-5 shrink-0 items-center justify-center text-muted-foreground">
                    <Icon size={14} />
                  </span>
                {/if}
                <div class="flex h-7 min-w-0 flex-1 flex-col gap-0.5 justify-center">
                  <span class="truncate text-xs font-medium text-foreground">{cmd.title}</span>
                  {#if cmd.description}
                    <span class="truncate text-[10px] text-muted-foreground">{cmd.description}</span>
                  {/if}
                </div>
                {#if cmd.shortcut}
                  <Command.Shortcut>
                    <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px]">
                      {cmd.shortcut}
                    </kbd>
                  </Command.Shortcut>
                {/if}
              </Command.Item>
            {/each}
          </Command.Group>
        {/each}
      </Command.List>
      <div
        class="flex h-9 items-center justify-between gap-3 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
      >
        <span class="font-medium">Recast</span>
        <div class="flex items-center gap-3">
          <span class="flex items-center gap-1">
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">↵</kbd>
            <span>Run</span>
          </span>
          <span class="flex items-center gap-1">
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">↑</kbd>
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">↓</kbd>
            <span>Navigate</span>
          </span>
          <span class="flex items-center gap-1">
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
            <span>Close</span>
          </span>
        </div>
      </div>
    </Command.Root>
  </Dialog.Content>
</Dialog.Root>
