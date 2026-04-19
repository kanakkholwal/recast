<script lang="ts">
  import SourceSelectorSkeleton from "$components/skeletons/SourceSelectorSkeleton.svelte";
  import { getDisplays, getWindows } from "$lib/ipc";
  import {
    AppWindow,
    Check,
    Monitor as MonitorIcon,
    RefreshCw,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Tabs from "@recast/ui/tabs";
  import { cn } from "@recast/ui/utils";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  type TargetSource = {
    type: "monitor" | "window";
    id: number;
    label: string;
    appName?: string;
    thumbnail: string | null;
    resolution?: string;
  };

  let sources: TargetSource[] = $state([]);
  let selectedSource: TargetSource | null = $state(null);
  let tab = $state<"monitor" | "window">("monitor");
  let isFetching = $state(true);

  onMount(() => {
    fetchSources();
  });

  async function fetchSources() {
    isFetching = true;
    try {
      const [displays, windows] = await Promise.all([
        getDisplays(),
        getWindows(),
      ]);
      const next: TargetSource[] = [];
      displays.forEach((d, i) =>
        next.push({
          type: "monitor",
          id: d.id,
          label: d.isPrimary ? "Primary Display" : `Display ${i + 1}`,
          thumbnail: d.thumbnail,
          resolution: `${d.width} × ${d.height}`,
        }),
      );
      windows.forEach((w) => {
        if (w.title?.trim()) {
          next.push({
            type: "window",
            id: w.id,
            label: w.title,
            appName: w.appName,
            thumbnail: w.thumbnail,
            resolution: `${w.width} × ${w.height}`,
          });
        }
      });
      sources = next;
      if (!selectedSource && sources.length > 0) selectedSource = sources[0];
    } catch (e) {
      console.error(e);
    } finally {
      isFetching = false;
    }
  }

  function confirmSelection() {
    if (!selectedSource) return;
    emit("source-selected", selectedSource);
    getCurrentWindow().close();
  }

  function closeWindow() {
    getCurrentWindow().close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeWindow();
    } else if (e.key === "Enter" && selectedSource) {
      e.preventDefault();
      confirmSelection();
    }
  }

  const monitorSources = $derived(sources.filter((s) => s.type === "monitor"));
  const windowSources = $derived(sources.filter((s) => s.type === "window"));
  const filteredSources = $derived(
    tab === "monitor" ? monitorSources : windowSources,
  );

  function isSelected(source: TargetSource) {
    return (
      selectedSource?.id === source.id && selectedSource?.type === source.type
    );
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="group/root flex h-screen w-full flex-col overflow-hidden select-none bg-background/80 backdrop-blur-3xl"
>
  <!-- Header -->
  <header
    class="group/header flex items-center justify-between border-b border-border-subtle px-4 h-10 shrink-0"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2">
      <span
        class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
      >
        Choose source
      </span>
    </div>
    <Button
      variant="ghost"
      size="icon-sm"
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      class="opacity-0 group-hover/root:opacity-100 transition-opacity"
      title="Close (Esc)"
    >
      <X size={11} strokeWidth={2.5} />
    </Button>
  </header>

  <!-- Tabs -->
  <div class="px-3 pt-3 pb-2 shrink-0">
    <Tabs.Root bind:value={tab} class="w-full">
      <Tabs.List
        class="grid w-full grid-cols-2 bg-muted/60 border border-border-subtle p-0.5"
      >
        <Tabs.Trigger
          value="monitor"
          class="gap-1.5 text-[11px] font-medium text-muted-foreground data-active:bg-background data-active:text-foreground data-active:border-border-subtle data-active:shadow-craft-sm"
        >
          <MonitorIcon size={12} />
          Screens
          {#if monitorSources.length > 0}
            <span
              class={cn(
                "rounded-sm px-1 py-px font-mono text-[10px] tabular-nums transition-colors",
                tab === "monitor"
                  ? "bg-primary/15 text-foreground"
                  : "bg-muted text-muted-foreground",
              )}
            >
              {monitorSources.length}
            </span>
          {/if}
        </Tabs.Trigger>
        <Tabs.Trigger
          value="window"
          class="gap-1.5 text-[11px] font-medium text-muted-foreground data-active:bg-background data-active:text-foreground data-active:border-border-subtle data-active:shadow-craft-sm"
        >
          <AppWindow size={12} />
          Windows
          {#if windowSources.length > 0}
            <span
              class={cn(
                "rounded-sm px-1 py-px font-mono text-[10px] tabular-nums transition-colors",
                tab === "window"
                  ? "bg-primary/15 text-foreground"
                  : "bg-muted text-muted-foreground",
              )}
            >
              {windowSources.length}
            </span>
          {/if}
        </Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto px-3 pb-3 scrollbar-transparent">
    {#if isFetching}
      <SourceSelectorSkeleton />
    {:else if filteredSources.length === 0}
      <div
        class="flex h-32 w-full flex-col items-center justify-center gap-2 rounded-md border border-dashed border-border bg-card/40"
      >
        {#if tab === "monitor"}
          <MonitorIcon size={18} class="text-muted-foreground" />
        {:else}
          <AppWindow size={18} class="text-muted-foreground" />
        {/if}
        <p class="text-[11px] font-medium text-foreground">
          No {tab === "monitor" ? "displays" : "windows"} found
        </p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-2">
        {#each filteredSources as source (source.type + source.id)}
          {@const selected = isSelected(source)}
          <button
            type="button"
            onclick={() => (selectedSource = source)}
            ondblclick={confirmSelection}
            onmousedown={(e) => e.stopPropagation()}
            class={cn(
              "group/tile relative flex flex-col overflow-hidden rounded-md border text-left transition-colors",
              "focus:outline-none focus:ring-1 focus:ring-ring",
              selected
                ? "border-primary bg-primary/10"
                : "border-border bg-card hover:bg-muted/50",
            )}
          >
            <!-- Thumbnail -->
            <div
              class="relative aspect-16/10 w-full overflow-hidden bg-muted/30"
            >
              {#if source.thumbnail}
                <img
                  src={source.thumbnail}
                  alt={source.label}
                  class="h-full w-full object-cover"
                  draggable="false"
                />
              {:else}
                <div
                  class="flex h-full w-full items-center justify-center text-muted-foreground"
                >
                  {#if source.type === "monitor"}
                    <MonitorIcon size={24} />
                  {:else}
                    <AppWindow size={24} />
                  {/if}
                </div>
              {/if}

              {#if selected}
                <div
                  class="absolute right-1.5 top-1.5 size-5 rounded-full bg-primary flex items-center justify-center shadow-craft-sm"
                >
                  <Check
                    size={11}
                    strokeWidth={3}
                    class="text-primary-foreground"
                  />
                </div>
              {/if}
            </div>

            <!-- Label -->
            <div class="px-2 py-1.5">
              <div
                class="truncate text-[11px] font-medium leading-tight text-foreground"
              >
                {source.label}
              </div>
              {#if source.resolution}
                <div
                  class="mt-0.5 text-[10px] font-mono tabular-nums text-muted-foreground"
                >
                  {source.resolution}
                </div>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer -->
  <footer
    class="flex items-center justify-between border-t border-border-subtle bg-background/50 px-3 h-11 shrink-0"
  >
    <Button
      onclick={fetchSources}
      disabled={isFetching}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="xs"
      class="gap-1.5"
    >
      <RefreshCw size={11} class={isFetching ? "animate-spin" : ""} />
      Rescan
    </Button>

    <div class="flex items-center gap-1.5">
      <Button
        onclick={closeWindow}
        onmousedown={(e) => e.stopPropagation()}
        variant="ghost"
        size="xs"
      >
        Cancel
      </Button>
      <Button
        onclick={confirmSelection}
        disabled={!selectedSource}
        onmousedown={(e) => e.stopPropagation()}
        variant="default"
        size="xs"
      >
        Select
      </Button>
    </div>
  </footer>
</div>
