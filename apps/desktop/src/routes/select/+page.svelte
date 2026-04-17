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
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { cn } from "@recast/ui/utils";

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
  class="flex h-screen w-full flex-col overflow-hidden bg-background/50 backdrop-blur-3xl text-foreground font-sans select-none"
>
  <!-- Header -->
  <div class="group/header flex items-center justify-between px-7 pt-7 pb-5 shrink-0" data-tauri-drag-region>
    <div class="space-y-1.5">
      <h1 class="text-2xl font-semibold tracking-tight text-foreground">
        Choose Source
      </h1>
      <p class="text-[11px] font-medium text-foreground/40 uppercase tracking-[0.15em]">Select what to capture</p>
    </div>
    <button
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      class="size-8 rounded-full flex items-center justify-center text-foreground/20 hover:text-foreground hover:bg-foreground/5 opacity-0 group-hover/header:opacity-100 transition-all duration-200"
    >
      <X size={14} strokeWidth={2.5} />
    </button>
  </div>

  <!-- Tabs -->
  <div
    class="mx-7 mb-6 flex items-center gap-1 rounded-[18px] bg-foreground/[0.03] p-1 shrink-0"
  >
    <button
      onclick={() => (tab = "monitor")}
      class="flex-1 flex items-center justify-center gap-2 rounded-[16px] py-2.5 text-[12px] font-medium tracking-tight transition-all duration-200
                {tab === 'monitor'
        ? 'bg-background text-foreground shadow-craft-md ring-1 ring-border-subtle'
        : 'text-foreground/40 hover:text-foreground/60 hover:bg-foreground/[0.02]'}"
    >
      <MonitorIcon size={14} strokeWidth={2} />
      Screens
      {#if monitorSources.length > 0}
        <span class="rounded-md bg-foreground/[0.05] px-1.5 py-0.5 text-[10px] text-foreground/40"
          >{monitorSources.length}</span
        >
      {/if}
    </button>
    <button
      onclick={() => (tab = "window")}
      class="flex-1 flex items-center justify-center gap-2 rounded-[16px] py-2.5 text-[12px] font-medium tracking-tight transition-all duration-200
                {tab === 'window'
        ? 'bg-background text-foreground shadow-craft-md ring-1 ring-border-subtle'
        : 'text-foreground/40 hover:text-foreground/60 hover:bg-foreground/[0.02]'}"
    >
      <AppWindow size={14} strokeWidth={2} />
      Windows
      {#if windowSources.length > 0}
        <span class="rounded-md bg-foreground/[0.05] px-1.5 py-0.5 text-[10px] text-foreground/40"
          >{windowSources.length}</span
        >
      {/if}
    </button>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto px-7 pb-5 scrollbar-transparent">
    {#if isFetching}
      <SourceSelectorSkeleton />
    {:else if filteredSources.length === 0}
      <div class="flex h-48 w-full flex-col items-center justify-center gap-4 bg-foreground/[0.02] rounded-3xl border border-border-subtle">
        <div
          class="size-12 rounded-2xl bg-foreground/5 flex items-center justify-center text-foreground/20"
        >
          {#if tab === "monitor"}
            <MonitorIcon size={24} strokeWidth={1.5} />
          {:else}
            <AppWindow size={24} strokeWidth={1.5} />
          {/if}
        </div>
        <p class="text-[13px] font-medium text-foreground/30">
          No {tab === "monitor" ? "displays" : "windows"} found
        </p>
      </div>
    {:else}
      <div
        class="grid gap-3.5 {tab === 'monitor' ? 'grid-cols-2' : 'grid-cols-2'}"
      >
        {#each filteredSources as source, i}
          <button
            onclick={() => (selectedSource = source)}
            class={cn(
              "group relative overflow-hidden rounded-[24px] border transition-all duration-300",
              isSelected(source)
              ? "border-primary/40 bg-primary/[0.02] shadow-craft-lg"
              : "border-border-subtle bg-card/40 hover:bg-card hover:border-border/40 hover:shadow-craft-md hover:scale-[1.01]"
            )}
            style="animation-delay: {i * 30}ms"
          >
            <!-- Thumbnail -->
            <div
              class="relative aspect-[16/10] w-full overflow-hidden bg-muted/20"
            >
              {#if source.thumbnail}
                <img
                  src={source.thumbnail}
                  alt={source.label}
                  class="h-full w-full object-cover transition-transform duration-700 group-hover:scale-105"
                  draggable="false"
                />
              {:else}
                <div
                  class="flex h-full w-full items-center justify-center text-foreground/5 transition-all group-hover:scale-110"
                >
                  {#if source.type === "monitor"}
                    <MonitorIcon size={40} strokeWidth={1} />
                  {:else}
                    <AppWindow size={40} strokeWidth={1} />
                  {/if}
                </div>
              {/if}

              <!-- Selection Ring / Check -->
              <div class={cn(
                "absolute inset-0 ring-1 ring-inset ring-primary/30 transition-all duration-300",
                isSelected(source) ? "opacity-100" : "opacity-0"
              )}></div>

              {#if isSelected(source)}
                <div
                  class="absolute right-4 top-4 size-7 rounded-full bg-primary flex items-center justify-center shadow-craft-md animate-in zoom-in-50 duration-300"
                >
                  <Check size={14} strokeWidth={3} class="text-primary-foreground" />
                </div>
              {/if}
            </div>

            <!-- Label -->
            <div class="px-5 py-4.5">
              <div
                class={cn(
                  "truncate text-[13px] font-semibold leading-tight transition-colors",
                  isSelected(source) ? "text-foreground" : "text-foreground/70"
                )}
              >
                {source.label}
              </div>
              {#if source.resolution}
                <div class="mt-1.5 text-[10px] font-medium text-foreground/30 uppercase tracking-wider">
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
    class="group/footer flex items-center justify-between px-7 py-6 shrink-0 border-t border-border-subtle"
  >
    <Button
      onclick={fetchSources}
      disabled={isFetching}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="sm"
      class="text-[11px] font-medium uppercase tracking-[0.15em] text-foreground/20 hover:text-foreground transition-all duration-300 opacity-0 group-hover/footer:opacity-100"
    >
      <RefreshCw size={12} strokeWidth={2.5} class={isFetching ? "animate-spin" : ""} />
      Rescan
    </Button>

    <div class="flex items-center gap-4">
      <button
        onclick={closeWindow}
        onmousedown={(e) => e.stopPropagation()}
        class="text-[13px] font-medium px-6 text-foreground/40 hover:text-foreground transition-all duration-200"
      >
        Cancel
      </button>
      <Button
        onclick={confirmSelection}
        disabled={!selectedSource}
        onmousedown={(e) => e.stopPropagation()}
        class="shadow-craft-lg rounded-[14px] px-8 font-semibold h-11 bg-foreground text-background hover:bg-foreground/90 transition-all duration-200"
      >
        Select Source
      </Button>
    </div>
  </div>
</div>

