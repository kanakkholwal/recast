<script lang="ts">
    import {
        AppWindow,
        CheckCircle2,
        Monitor as Display,
        LayoutTemplate,
        Minus,
        RefreshCw,
        X,
    } from "@lucide/svelte";
    import SourceSelectorSkeleton from "$components/skeletons/SourceSelectorSkeleton.svelte";
    import {
        getDisplays,
        getWindows,
        type DisplayInfo,
        type WindowInfo,
    } from "$lib/ipc";
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
    let selectorTab: "monitor" | "window" = $state("monitor");
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
            const newSources: TargetSource[] = [];
            displays.forEach((d, i) =>
                newSources.push({
                    type: "monitor",
                    id: d.id,
                    label: d.isPrimary
                        ? "Primary Display"
                        : `Display ${i + 1}`,
                    thumbnail: d.thumbnail,
                    resolution: `${d.width} × ${d.height}`,
                }),
            );
            windows.forEach((w) => {
                if (w.title?.trim()) {
                    newSources.push({
                        type: "window",
                        id: w.id,
                        label: w.title,
                        appName: w.appName,
                        thumbnail: w.thumbnail,
                        resolution: `${w.width} × ${w.height}`,
                    });
                }
            });
            sources = newSources;
            if (!selectedSource && sources.length > 0)
                selectedSource = sources[0];
        } catch (e) {
            console.error(e);
        } finally {
            isFetching = false;
        }
    }

    async function confirmSelection() {
        if (selectedSource) {
            await emit("source-selected", selectedSource);
            await getCurrentWindow().close();
        }
    }

    async function closeApp() {
        await getCurrentWindow().close();
    }
    async function minimizeWindow() {
        await getCurrentWindow().minimize();
    }

    let monitorSources = $derived(sources.filter((s) => s.type === "monitor"));
    let windowSources = $derived(sources.filter((s) => s.type === "window"));
    let filteredSources = $derived(
        selectorTab === "monitor" ? monitorSources : windowSources,
    );
</script>

<div
    class="flex h-screen w-full flex-col overflow-hidden bg-background font-sans text-foreground selection:bg-primary/10 selection:text-primary rounded-xl border border-border/50 shadow-2xl"
>
    <!-- Title bar -->
    <div
        class="flex shrink-0 items-center justify-between px-5 pt-4 pb-2"
        data-tauri-drag-region
    >
        <div class="flex shrink-0 items-center gap-2">
            <button
                onclick={closeApp}
                class="group flex h-3 w-3 items-center justify-center rounded-full bg-destructive/80 transition-colors hover:bg-destructive"
                title="Close"
            >
                <X
                    size={8}
                    class="text-destructive-foreground opacity-0 group-hover:opacity-100 transition-opacity"
                    strokeWidth={3}
                />
            </button>
            <button
                onclick={minimizeWindow}
                class="group flex h-3 w-3 items-center justify-center rounded-full bg-yellow-400/80 transition-colors hover:bg-yellow-500"
                title="Minimize"
            >
                <Minus
                    size={8}
                    class="text-orange-900 opacity-0 group-hover:opacity-100 transition-opacity"
                    strokeWidth={3}
                />
            </button>
        </div>
        <span
            class="pointer-events-none select-none text-sm font-semibold tracking-tight"
            data-tauri-drag-region>Choose Source</span
        >
        <button
            onclick={closeApp}
            class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground active:scale-95"
            title="Close"
            aria-label="Close"
        >
            <X size={16} strokeWidth={2.5} />
        </button>
    </div>

    <!-- Segmented tabs -->
    <div
        class="mx-5 mb-4 mt-2 flex shrink-0 select-none items-center gap-1 rounded-xl bg-muted p-1 border border-border"
    >
        <button
            onclick={() => (selectorTab = "monitor")}
            class="flex-1 rounded-lg py-1.5 text-xs font-medium transition-all {selectorTab ===
            'monitor'
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:text-foreground'}"
        >
            Screens <span class="ml-1 text-[10px] text-muted-foreground/70"
                >{monitorSources.length || ""}</span
            >
        </button>
        <button
            onclick={() => (selectorTab = "window")}
            class="flex-1 rounded-lg py-1.5 text-xs font-medium transition-all {selectorTab ===
            'window'
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:text-foreground'}"
        >
            Windows <span class="ml-1 text-[10px] text-muted-foreground/70"
                >{windowSources.length || ""}</span
            >
        </button>
    </div>

    <!-- Grid -->
    <div class="flex-1 select-none overflow-y-auto px-5 pb-4 custom-scrollbar">
        {#if isFetching}
            <SourceSelectorSkeleton />
        {:else if filteredSources.length === 0}
            <div
                class="flex h-44 w-full flex-col items-center justify-center gap-3 animate-in fade-in zoom-in-95"
            >
                <div
                    class="flex h-12 w-12 items-center justify-center rounded-xl border border-dashed border-border bg-muted/50 text-muted-foreground"
                >
                    <LayoutTemplate size={24} strokeWidth={1.5} />
                </div>
                <p class="text-xs text-muted-foreground">
                    No {selectorTab === "monitor" ? "displays" : "windows"} found
                </p>
            </div>
        {:else}
            <div
                class="grid gap-4 {selectorTab === 'monitor'
                    ? 'grid-cols-2'
                    : 'grid-cols-3'}"
            >
                {#each filteredSources as source, i}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                        onclick={() => (selectedSource = source)}
                        class="group cursor-pointer overflow-hidden rounded-xl border bg-card transition-all duration-300 animate-in slide-in-from-bottom-2 fade-in {selectedSource?.id ===
                            source.id && selectedSource?.type === source.type
                            ? 'border-primary ring-1 ring-primary/20 shadow-md bg-primary/5'
                            : 'border-border hover:border-border/80 hover:shadow-sm hover:-translate-y-0.5'}"
                        style="animation-delay: {i * 40}ms;"
                    >
                        <!-- Thumbnail -->
                        <div
                            class="relative aspect-video w-full overflow-hidden bg-muted flex items-center justify-center border-b border-border/50"
                        >
                            {#if source.thumbnail}
                                <img
                                    src={source.thumbnail}
                                    alt={source.label}
                                    class="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
                                    draggable="false"
                                />
                            {:else}
                                <div
                                    class="flex h-full w-full items-center justify-center text-muted-foreground/30"
                                >
                                    {#if source.type === "monitor"}
                                        <Display size={32} strokeWidth={1.5} />
                                    {:else}
                                        <AppWindow
                                            size={32}
                                            strokeWidth={1.5}
                                        />
                                    {/if}
                                </div>
                            {/if}

                            {#if selectedSource?.id === source.id && selectedSource?.type === source.type}
                                <div
                                    class="absolute right-2 top-2 flex h-6 w-6 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg animate-in zoom-in-50 duration-200"
                                >
                                    <CheckCircle2 size={14} strokeWidth={3} />
                                </div>
                            {/if}
                        </div>
                        <!-- Label -->
                        <div class="px-3.5 py-2.5">
                            <div
                                class="truncate text-xs font-medium text-foreground leading-tight"
                                title={source.label}
                            >
                                {source.label}
                            </div>
                            {#if source.resolution}
                                <div
                                    class="mt-0.5 text-[10px] font-mono tracking-tight text-muted-foreground"
                                >
                                    {source.resolution}
                                </div>
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>
        {/if}
    </div>

    <!-- Footer -->
    <div
        class="flex shrink-0 select-none items-center justify-between border-t border-border bg-background px-5 py-4"
    >
        <button
            onclick={fetchSources}
            disabled={isFetching}
            class="group flex items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
            aria-label="Refresh"
        >
            <RefreshCw
                size={14}
                class={isFetching
                    ? "animate-spin"
                    : "group-hover:rotate-180 transition-transform duration-500"}
            />
            Refresh
        </button>
        <div class="flex items-center gap-2.5">
            <button
                onclick={closeApp}
                class="rounded-lg px-4 py-2 text-xs font-medium text-muted-foreground transition-all hover:bg-muted hover:text-foreground active:scale-95"
            >
                Cancel
            </button>
            <button
                onclick={confirmSelection}
                disabled={!selectedSource}
                class="rounded-lg bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow-sm transition-all hover:bg-primary/90 hover:shadow disabled:opacity-50 disabled:cursor-not-allowed active:scale-95"
            >
                Confirm
            </button>
        </div>
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar {
        width: 5px;
    }
    .custom-scrollbar::-webkit-scrollbar-track {
        background: transparent;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb {
        background: var(--muted-foreground);
        opacity: 0.3;
        border-radius: 100px;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb:hover {
        background: var(--muted-foreground);
        opacity: 0.5;
    }
</style>
