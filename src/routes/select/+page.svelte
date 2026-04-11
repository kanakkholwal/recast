<script lang="ts">
  import SourceSelectorSkeleton from "$components/skeletons/SourceSelectorSkeleton.svelte";
  import { Button } from "$components/ui/button";
  import { getDisplays, getWindows } from "$lib/ipc";
  import {
    AppWindow,
    Check,
    Monitor as MonitorIcon,
    RefreshCw,
    X,
  } from "@lucide/svelte";
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
  let tab: "monitor" | "window" = $state("monitor");
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

<div
  class="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground font-sans"
>
  <!-- Header -->
  <div class="flex items-center justify-between px-4 pt-3 pb-2 shrink-0">
    <span
      class="text-[13px] font-semibold tracking-tight text-card-foreground/90"
      >Choose Source</span
    >
    <button
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      class="size-6 rounded-md flex items-center justify-center text-card-foreground/30 hover:text-card-foreground/70 hover:bg-card/8 transition-colors"
    >
      <X size={14} strokeWidth={2} />
    </button>
  </div>

  <!-- Tabs -->
  <div
    class="mx-4 mb-3 flex items-center gap-0.5 rounded-lg bg-muted p-0.5 shrink-0"
  >
    <button
      onclick={() => (tab = "monitor")}
      class="flex-1 flex items-center justify-center gap-1.5 rounded-md py-1.5 text-[11px] font-medium transition-all
                {tab === 'monitor'
        ? 'bg-card text-card-foreground shadow-sm'
        : 'text-card-foreground/40 hover:text-card-foreground/60'}"
    >
      <MonitorIcon size={12} />
      Screens
      {#if monitorSources.length > 0}
        <span class="text-[9px] text-card-foreground"
          >{monitorSources.length}</span
        >
      {/if}
    </button>
    <button
      onclick={() => (tab = "window")}
      class="flex-1 flex items-center justify-center gap-1.5 rounded-md py-1.5 text-[11px] font-medium transition-all
                {tab === 'window'
        ? 'bg-card text-card-foreground shadow-sm'
        : 'text-muted-foreground hover:text-card-foreground'}"
    >
      <AppWindow size={12} />
      Windows
      {#if windowSources.length > 0}
        <span class="text-[9px] text-card-foreground/30"
          >{windowSources.length}</span
        >
      {/if}
    </button>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto px-4 pb-3 custom-scrollbar">
    {#if isFetching}
      <SourceSelectorSkeleton />
    {:else if filteredSources.length === 0}
      <div class="flex h-32 w-full flex-col items-center justify-center gap-2">
        <div
          class="size-10 rounded-lg bg-card/5 flex items-center justify-center text-card-foreground/20"
        >
          {#if tab === "monitor"}
            <MonitorIcon size={20} strokeWidth={1.5} />
          {:else}
            <AppWindow size={20} strokeWidth={1.5} />
          {/if}
        </div>
        <p class="text-[11px] text-card-foreground/30">
          No {tab === "monitor" ? "displays" : "windows"} found
        </p>
      </div>
    {:else}
      <div
        class="grid gap-2 {tab === 'monitor' ? 'grid-cols-2' : 'grid-cols-3'}"
      >
        {#each filteredSources as source, i}
          <button
            onclick={() => (selectedSource = source)}
            class="group relative overflow-hidden rounded-lg border text-left transition-all duration-200
                            {isSelected(source)
              ? 'border-border bg-primary ring-1 ring-primary/10'
              : 'border-card bg-card hover:border-card/12 hover:bg-card/5'}"
            style="animation-delay: {i * 30}ms"
          >
            <!-- Thumbnail -->
            <div
              class="relative aspect-video w-full overflow-hidden bg-card/30"
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
                  class="flex h-full w-full items-center justify-center text-card-foreground/10"
                >
                  {#if source.type === "monitor"}
                    <MonitorIcon size={24} strokeWidth={1.5} />
                  {:else}
                    <AppWindow size={24} strokeWidth={1.5} />
                  {/if}
                </div>
              {/if}

              {#if isSelected(source)}
                <div
                  class="absolute right-1.5 top-1.5 size-5 rounded-full bg-card flex items-center justify-center shadow-lg"
                >
                  <Check size={11} strokeWidth={3} class="text-black" />
                </div>
              {/if}
            </div>

            <!-- Label -->
            <div class="px-2.5 py-2">
              <div
                class="truncate text-[11px] font-medium text-card-foreground/80 leading-tight"
              >
                {source.label}
              </div>
              {#if source.resolution}
                <div
                  class="mt-0.5 text-[9px] font-mono text-card-foreground/25"
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
  <div
    class="flex items-center justify-between px-4 py-3 border-t border-border/6 shrink-0"
  >
    <Button
      onclick={fetchSources}
      disabled={isFetching}
      onmousedown={(e) => e.stopPropagation()}
       variant="ghost"
        size="sm"
    >
      <RefreshCw size={12} class={isFetching ? "animate-spin" : ""} />
      Refresh
    </Button>
    <div class="flex items-center gap-2">
      <Button
        onclick={closeWindow}
        onmousedown={(e) => e.stopPropagation()}
        variant="ghost"
        size="sm"
      >
        Cancel
      </Button>
      <Button
        onclick={confirmSelection}
        disabled={!selectedSource}
        onmousedown={(e) => e.stopPropagation()}
        variant="default"
        size="sm"
      >
        Confirm
      </Button>
    </div>
  </div>
</div>

<style>
  :global(.custom-scrollbar)::-webkit-scrollbar {
    width: 4px;
  }
  :global(.custom-scrollbar)::-webkit-scrollbar-track {
    background: transparent;
  }
  :global(.custom-scrollbar)::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 100px;
  }
  :global(.custom-scrollbar)::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
