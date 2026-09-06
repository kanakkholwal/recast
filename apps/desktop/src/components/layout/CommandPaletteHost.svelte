<script lang="ts">
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import { CornerDownLeft, Search, X } from "@recast/icons";
import { Kbd, KbdGroup } from "@recast/ui/kbd";
import { cn } from "@recast/ui/utils";
import { onMount, tick } from "svelte";
import { cubicOut } from "svelte/easing";
import { fade, scale } from "svelte/transition";
import { buildGlobalCommands } from "$lib/commands";
import { commandPalette, type PaletteCommand } from "$lib/stores/command-palette.svelte";
import { groupCommands, highlight, rankCommands } from "./command-palette-host.logic";

let query = $state("");
let selectedIndex = $state(0);
let inputRef = $state<HTMLInputElement | null>(null);
let listRef = $state<HTMLDivElement | null>(null);
let contentHeight = $state(0);

// Safe on every mount, since registerMany dedupes by id; the open shortcut lives in the central registry.
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

const grouped = $derived.by<[string, PaletteCommand[]][]>(() => groupCommands(filtered, query));

// Mirrors render order, so the selected index always points at a real button.
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
		selectedIndex = (selectedIndex - 1 + flatItems.length) % flatItems.length;
		scrollSelectedIntoView();
	} else if (e.key === "Enter") {
		e.preventDefault();
		const cmd = flatItems[selectedIndex];
		if (cmd) runCommand(cmd);
	}
}

function scrollSelectedIntoView() {
	if (!listRef) return;
	const el = listRef.querySelector<HTMLElement>(`[data-index="${selectedIndex}"]`);
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

function clearQuery() {
	query = "";
	inputRef?.focus();
}
</script>

{#if commandPalette.open}
  <div
    class="fixed inset-0 z-60 bg-background/70 backdrop-blur-sm"
    transition:fade={{ duration: motionDuration(150) }}
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
      class="relative w-full max-w-xl transform-gpu overflow-hidden rounded-xl border border-border/60 bg-popover/95 shadow-(--shadow-craft-xl) ring-1 ring-border/50 backdrop-blur-xl"
      role="document"
      transition:scale={{ duration: motionDuration(220), start: 0.96, easing: cubicOut }}
    >
      <div class="flex items-center gap-2.5 border-b border-border/50 px-3.5">
        <Search class="size-4 shrink-0 text-muted-foreground/70" />
        <input
          bind:this={inputRef}
          bind:value={query}
          class="command-palette-input flex h-12 w-full bg-transparent text-sm tracking-normal text-foreground placeholder:text-muted-foreground/70 focus:outline-none"
          placeholder="Type a command or search…"
          aria-label="Search commands"
          role="combobox"
          aria-expanded="true"
          aria-controls="command-palette-list"
          aria-activedescendant={flatItems.length
            ? `command-palette-item-${selectedIndex}`
            : undefined}
          autocomplete="off"
          spellcheck="false"
        />
        {#if query}
          <button
            type="button"
            onclick={clearQuery}
            aria-label="Clear search"
            class="flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <X class="size-3.5" />
          </button>
        {/if}
        <Kbd class="hidden shrink-0 sm:inline-flex">Esc</Kbd>
      </div>

      <div
        class="overflow-hidden transition-[height] duration-300 ease-out"
        style="height: {contentHeight}px"
      >
        <div bind:clientHeight={contentHeight}>
          {#if flatItems.length > 0}
            <div
              bind:this={listRef}
              id="command-palette-list"
              role="listbox"
              aria-label="Commands"
              class="scrollbar-transparent max-h-96 overflow-y-auto p-1.5"
              style="mask-image: linear-gradient(to bottom, black calc(100% - 12px), transparent); -webkit-mask-image: linear-gradient(to bottom, black calc(100% - 12px), transparent);"
            >
              {#each grouped as [category, cmds] (category)}
                <div
                  class="sticky top-0 z-10 flex items-center bg-popover px-3 pb-1 pt-3"
                >
                  <span
                    class="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground/70"
                  >
                    {category}
                  </span>
                </div>
                {#each cmds as cmd (cmd.id)}
                  {@const i = indexOfCmd(cmd)}
                  {@const Icon = cmd.icon}
                  {@const active = i === selectedIndex}
                  <button
                    type="button"
                    role="option"
                    id="command-palette-item-{i}"
                    aria-selected={active}
                    data-index={i}
                    class={cn(
                      // scroll-mt clears the sticky category header on ↑/↓ nav.
                      "group flex w-full scroll-mt-8 items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors duration-100",
                      active
                        ? "bg-foreground/8 text-foreground"
                        : "text-foreground/80 hover:bg-foreground/5",
                    )}
                    onclick={() => runCommand(cmd)}
                    onmouseenter={() => (selectedIndex = i)}
                  >
                    {#if Icon}
                      <Icon
                        size={18}
                        class={cn("shrink-0", active ? "text-foreground" : "text-muted-foreground")}
                      />
                    {:else}
                      <span
                        class="flex size-4.5 shrink-0 items-center justify-center text-muted-foreground"
                      >
                        <span class="size-1.5 rounded-full bg-current opacity-40"></span>
                      </span>
                    {/if}
                    <span class="min-w-0 flex-1 truncate text-[13px] font-medium">
                      {#each highlight(cmd.title, query) as part, k (k)}
                        {#if part.hl}
                          <span class="font-semibold text-foreground">{part.text}</span>
                        {:else}{part.text}{/if}
                      {/each}
                    </span>
                    {#if cmd.shortcut}
                      <span class="hidden shrink-0 items-center gap-1 sm:flex">
                        {#each cmd.shortcut.split(/[+\s]+/).filter(Boolean) as key (key)}
                          <Kbd>{key}</Kbd>
                        {/each}
                      </span>
                    {/if}
                  </button>
                {/each}
              {/each}
            </div>
          {:else if query}
            <div class="flex flex-col items-center px-4 py-10 text-center">
              <span
                class="flex size-9 items-center justify-center rounded-lg border border-border/40 bg-muted/40 text-muted-foreground"
              >
                <Search class="size-4" />
              </span>
              <p class="mt-3 text-xs font-medium text-foreground">
                No command matches “{query}”
              </p>
              <p class="mt-1 text-[11px] text-muted-foreground">
                Try a shorter term, or the name of the page you want.
              </p>
            </div>
          {/if}
        </div>
      </div>

      <div
        class="flex items-center justify-between gap-3 border-t border-border/50 bg-muted/25 px-3.5 py-2 text-[11px] text-muted-foreground"
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
