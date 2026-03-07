<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
    import { onMount } from "svelte";

    type DisplayInfo = {
        id: number;
        name: string;
        width: number;
        height: number;
        is_primary: boolean;
        thumbnail: string | null;
    };

    type WindowInfo = {
        id: number;
        pid: number;
        app_name: string;
        title: string;
        width: number;
        height: number;
        thumbnail: string | null;
    };

    type TargetType = "monitor" | "window";

    type TargetSource = {
        type: TargetType;
        id: number;
        label: string;
        appName?: string;
        thumbnail: string | null;
        resolution?: string;
    };

    let sources: TargetSource[] = $state([]);
    let selectedSource: TargetSource | null = $state(null);

    let isRecording = $state(false);
    let recordingStartTime: number | null = $state(null);
    let now = $state(Date.now());
    let timerInterval: number;

    let isSelectorOpen = $state(false);
    let selectorTab: "monitor" | "window" = $state("monitor");
    let isFetching = $state(false);
    let isAnimating = $state(false);

    const PILL_WIDTH = 360;
    const PILL_HEIGHT = 54;
    const EXPANDED_WIDTH = 720;
    const EXPANDED_HEIGHT = 540;

    onMount(() => {
        timerInterval = window.setInterval(() => {
            if (isRecording) now = Date.now();
        }, 1000);

        // Auto-fetch sources on mount
        fetchSources();

        return () => window.clearInterval(timerInterval);
    });

    async function fetchSources() {
        isFetching = true;
        try {
            const [displays, windows] = await Promise.all([
                invoke<DisplayInfo[]>("get_displays"),
                invoke<WindowInfo[]>("get_windows"),
            ]);

            const newSources: TargetSource[] = [];

            displays.forEach((d, i) => {
                newSources.push({
                    type: "monitor",
                    id: d.id,
                    label: d.is_primary
                        ? `Primary Display`
                        : `Display ${i + 1}`,
                    thumbnail: d.thumbnail,
                    resolution: `${d.width}×${d.height}`,
                });
            });

            windows.forEach((w) => {
                if (w.title && w.title.trim().length > 0) {
                    newSources.push({
                        type: "window",
                        id: w.id,
                        label: w.title,
                        appName: w.app_name,
                        thumbnail: w.thumbnail,
                        resolution: `${w.width}×${w.height}`,
                    });
                }
            });

            sources = newSources;
            if (!selectedSource && sources.length > 0) {
                selectedSource = sources[0];
            }
        } catch (e) {
            console.error("Failed to load sources", e);
        } finally {
            isFetching = false;
        }
    }

    async function openSelector() {
        if (isRecording || isAnimating) return;
        isAnimating = true;

        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(EXPANDED_WIDTH, EXPANDED_HEIGHT));
        await win.center();

        await new Promise((r) => setTimeout(r, 60));
        isSelectorOpen = true;
        setTimeout(() => {
            isAnimating = false;
        }, 500);

        await fetchSources();
    }

    async function closeSelector() {
        if (isAnimating) return;
        isAnimating = true;
        isSelectorOpen = false;

        setTimeout(async () => {
            const win = getCurrentWindow();
            await win.setSize(new LogicalSize(PILL_WIDTH, PILL_HEIGHT));
            await win.center();
            isAnimating = false;
        }, 420);
    }

    async function closeApp() {
        const win = getCurrentWindow();
        await win.close();
    }

    function selectSource(source: TargetSource) {
        selectedSource = source;
    }

    async function confirmSelection() {
        await closeSelector();
    }

    async function toggleRecording() {
        if (isRecording) {
            try {
                const filePath = await invoke<string>("stop_recording");
                isRecording = false;
                recordingStartTime = null;
                console.log(`Recording saved to: ${filePath}`);
            } catch (e) {
                console.error("Failed to stop recording", e);
            }
        } else {
            if (!selectedSource) return;
            try {
                await invoke("start_recording", {
                    targetType: selectedSource.type,
                    targetId: selectedSource.id,
                });
                isRecording = true;
                now = Date.now();
                recordingStartTime = now;
            } catch (e) {
                console.error("Failed to start recording", e);
            }
        }
    }

    let elapsedSeconds = $derived(
        isRecording && recordingStartTime
            ? Math.floor((now - recordingStartTime) / 1000)
            : 0,
    );

    let formattedTime = $derived(
        `${Math.floor(elapsedSeconds / 60)
            .toString()
            .padStart(
                2,
                "0",
            )}:${(elapsedSeconds % 60).toString().padStart(2, "0")}`,
    );

    let monitorSources = $derived(sources.filter((s) => s.type === "monitor"));
    let windowSources = $derived(sources.filter((s) => s.type === "window"));
    let filteredSources = $derived(
        selectorTab === "monitor" ? monitorSources : windowSources,
    );
</script>

<!-- Outer shell fills entire Tauri window -->
<main
    class="w-screen h-screen font-sans bg-transparent select-none overflow-hidden"
>
    <!-- ═══════════════════════════════════════════════════════════ -->
    <!-- DYNAMIC ISLAND CONTAINER                                   -->
    <!-- ═══════════════════════════════════════════════════════════ -->
    <div
        class="w-full h-full relative flex flex-col overflow-hidden"
        class:island-pill={!isSelectorOpen}
        class:island-expanded={isSelectorOpen}
        data-tauri-drag-region
    >
        <!-- ─────────────────────────────────────────────────────── -->
        <!-- PILL STATE                                              -->
        <!-- ─────────────────────────────────────────────────────── -->
        {#if !isSelectorOpen}
            <div
                class="flex items-center justify-between w-full h-full px-3 gap-2"
                style="animation: pillFadeIn 0.3s ease-out both 0.12s;"
            >
                <!-- Close button -->
                <button
                    onclick={closeApp}
                    class="w-[26px] h-[26px] rounded-full flex items-center justify-center bg-white/[0.05] hover:bg-red-500/20 text-zinc-500 hover:text-red-400 transition-all flex-shrink-0 cursor-pointer"
                    title="Close Trace"
                >
                    <svg
                        width="11"
                        height="11"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        ><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg
                    >
                </button>

                <!-- Divider -->
                <div class="w-px h-4 bg-white/[0.06] flex-shrink-0"></div>

                <!-- Status dot + Source selector -->
                <div class="flex items-center gap-2 min-w-0 flex-1">
                    <div
                        class="w-[7px] h-[7px] rounded-full flex-shrink-0 {isRecording
                            ? 'bg-red-500 shadow-[0_0_8px_rgba(239,68,68,0.7)] animate-pulse'
                            : 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.4)]'}"
                    ></div>

                    <button
                        disabled={isRecording}
                        onclick={openSelector}
                        class="bg-transparent text-zinc-300 text-[12.5px] font-medium tracking-wide outline-none cursor-pointer hover:text-white transition-colors disabled:opacity-40 text-left truncate min-w-0"
                    >
                        {#if selectedSource}
                            {selectedSource.label}
                        {:else}
                            Select Source
                        {/if}
                    </button>
                </div>

                <!-- Timer -->
                <span
                    class="text-zinc-500 font-mono text-[11.5px] tracking-[0.1em] tabular-nums w-[40px] text-right flex-shrink-0"
                >
                    {formattedTime}
                </span>

                <!-- Divider -->
                <div class="w-px h-4 bg-white/[0.06] flex-shrink-0"></div>

                <!-- Record button -->
                <button
                    onclick={toggleRecording}
                    class="w-[30px] h-[30px] rounded-full flex items-center justify-center transition-all duration-200 flex-shrink-0 cursor-pointer {isRecording
                        ? 'bg-red-500/15 hover:bg-red-500/25 ring-1 ring-red-500/30'
                        : 'bg-white/[0.05] hover:bg-white/[0.1] ring-1 ring-white/[0.08]'}"
                    title={isRecording ? "Stop Recording" : "Start Recording"}
                >
                    {#if isRecording}
                        <div
                            class="w-[10px] h-[10px] bg-red-500 rounded-[3px] shadow-[0_0_10px_rgba(239,68,68,0.5)]"
                        ></div>
                    {:else}
                        <div
                            class="w-[12px] h-[12px] rounded-full bg-gradient-to-br from-violet-500 to-fuchsia-500 shadow-[0_0_10px_rgba(139,92,246,0.4)]"
                        ></div>
                    {/if}
                </button>
            </div>
        {/if}

        <!-- ─────────────────────────────────────────────────────── -->
        <!-- EXPANDED STATE — Source Selection Grid                   -->
        <!-- ─────────────────────────────────────────────────────── -->
        {#if isSelectorOpen}
            <div
                class="w-full h-full flex flex-col"
                style="animation: expandFadeIn 0.35s ease-out both 0.1s;"
            >
                <!-- Header bar -->
                <div
                    class="flex items-center justify-between px-5 pt-4 pb-0"
                    data-tauri-drag-region
                >
                    <div class="flex items-center gap-2.5">
                        <!-- Close app -->
                        <button
                            onclick={closeApp}
                            class="w-3 h-3 rounded-full bg-[#ff5f57] opacity-80 hover:opacity-100 transition-opacity cursor-pointer"
                            title="Close"
                        ></button>
                        <!-- Minimize (collapse) -->
                        <button
                            onclick={closeSelector}
                            class="w-3 h-3 rounded-full bg-[#febc2e] opacity-80 hover:opacity-100 transition-opacity cursor-pointer"
                            title="Minimize"
                        ></button>
                        <!-- Placeholder green dot -->
                        <div
                            class="w-3 h-3 rounded-full bg-[#28c840] opacity-40"
                        ></div>
                    </div>

                    <h2
                        class="text-[14px] font-semibold text-white/80 tracking-tight"
                        data-tauri-drag-region
                    >
                        Choose Source
                    </h2>

                    <button
                        onclick={closeSelector}
                        title="Close"
                        class="w-7 h-7 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] flex items-center justify-center text-zinc-500 hover:text-white transition-all cursor-pointer"
                    >
                        <svg
                            width="13"
                            height="13"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.5"
                            stroke-linecap="round"
                            ><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg
                        >
                    </button>
                </div>

                <!-- Segmented tabs -->
                <div
                    class="flex items-center gap-1 mx-5 mt-4 mb-3 p-[3px] rounded-xl bg-white/[0.04] ring-1 ring-white/[0.05]"
                >
                    <button
                        onclick={() => (selectorTab = "monitor")}
                        class="flex-1 py-[7px] text-[12.5px] font-medium rounded-[10px] transition-all duration-200 cursor-pointer {selectorTab ===
                        'monitor'
                            ? 'bg-white/[0.1] text-white shadow-sm'
                            : 'text-zinc-500 hover:text-zinc-300'}"
                    >
                        Screens
                        {#if monitorSources.length > 0}
                            <span
                                class="ml-1 text-[10px] {selectorTab ===
                                'monitor'
                                    ? 'text-zinc-400'
                                    : 'text-zinc-600'}"
                                >{monitorSources.length}</span
                            >
                        {/if}
                    </button>
                    <button
                        onclick={() => (selectorTab = "window")}
                        class="flex-1 py-[7px] text-[12.5px] font-medium rounded-[10px] transition-all duration-200 cursor-pointer {selectorTab ===
                        'window'
                            ? 'bg-white/[0.1] text-white shadow-sm'
                            : 'text-zinc-500 hover:text-zinc-300'}"
                    >
                        Windows
                        {#if windowSources.length > 0}
                            <span
                                class="ml-1 text-[10px] {selectorTab ===
                                'window'
                                    ? 'text-zinc-400'
                                    : 'text-zinc-600'}"
                                >{windowSources.length}</span
                            >
                        {/if}
                    </button>
                </div>

                <!-- Sources grid -->
                <div class="flex-1 overflow-y-auto px-5 py-2 custom-scrollbar">
                    {#if isFetching}
                        <div
                            class="w-full h-full flex items-center justify-center"
                        >
                            <div class="flex flex-col items-center gap-3">
                                <div
                                    class="w-5 h-5 border-2 border-violet-500/50 border-t-transparent rounded-full animate-spin"
                                ></div>
                                <span
                                    class="text-[11px] text-zinc-600 tracking-wide"
                                    >Scanning sources…</span
                                >
                            </div>
                        </div>
                    {:else if filteredSources.length === 0}
                        <div
                            class="w-full h-44 flex flex-col items-center justify-center"
                        >
                            <div
                                class="w-10 h-10 rounded-xl bg-white/[0.03] ring-1 ring-white/[0.05] flex items-center justify-center mb-3"
                            >
                                {#if selectorTab === "monitor"}
                                    <svg
                                        class="text-zinc-600 w-5 h-5"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.5"
                                        ><rect
                                            width="20"
                                            height="14"
                                            x="2"
                                            y="3"
                                            rx="2"
                                        /><path d="M8 21h8" /><path
                                            d="M12 17v4"
                                        /></svg
                                    >
                                {:else}
                                    <svg
                                        class="text-zinc-600 w-5 h-5"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.5"
                                        ><rect
                                            width="18"
                                            height="18"
                                            x="3"
                                            y="3"
                                            rx="2"
                                        /><path d="M3 9h18" /><path
                                            d="M9 21V9"
                                        /></svg
                                    >
                                {/if}
                            </div>
                            <p class="text-zinc-500 text-[12px]">
                                No {selectorTab === "monitor"
                                    ? "displays"
                                    : "windows"} found
                            </p>
                        </div>
                    {:else}
                        <div
                            class="grid gap-3 {selectorTab === 'monitor'
                                ? 'grid-cols-2'
                                : 'grid-cols-3'}"
                        >
                            {#each filteredSources as source, i}
                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                <!-- svelte-ignore a11y_no_static_element_interactions -->
                                <div
                                    onclick={() => selectSource(source)}
                                    class="group cursor-pointer rounded-xl transition-all duration-200 overflow-hidden source-card
                                        {selectedSource?.id === source.id &&
                                    selectedSource?.type === source.type
                                        ? 'ring-[1.5px] ring-violet-500/60 bg-violet-500/[0.08] shadow-[0_0_20px_rgba(139,92,246,0.08)]'
                                        : 'ring-1 ring-white/[0.05] bg-white/[0.02] hover:ring-white/[0.1] hover:bg-white/[0.04]'}"
                                    style="animation: cardPop 0.25s ease-out both; animation-delay: {i *
                                        0.035}s;"
                                >
                                    <!-- Thumbnail area -->
                                    <div
                                        class="w-full aspect-video bg-[#0a0a0c] overflow-hidden relative"
                                    >
                                        {#if source.thumbnail}
                                            <img
                                                src={source.thumbnail}
                                                alt={source.label}
                                                class="w-full h-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
                                                draggable="false"
                                            />
                                        {:else}
                                            <div
                                                class="w-full h-full flex items-center justify-center"
                                            >
                                                {#if source.type === "monitor"}
                                                    <svg
                                                        class="text-zinc-700 w-8 h-8"
                                                        viewBox="0 0 24 24"
                                                        fill="none"
                                                        stroke="currentColor"
                                                        stroke-width="1.2"
                                                        ><rect
                                                            width="20"
                                                            height="14"
                                                            x="2"
                                                            y="3"
                                                            rx="2"
                                                        /><path
                                                            d="M8 21h8"
                                                        /><path
                                                            d="M12 17v4"
                                                        /></svg
                                                    >
                                                {:else}
                                                    <div
                                                        class="px-2 py-1 rounded bg-white/[0.03] text-zinc-700 font-semibold text-[10px] uppercase tracking-wider"
                                                    >
                                                        {source.appName?.substring(
                                                            0,
                                                            6,
                                                        ) || "APP"}
                                                    </div>
                                                {/if}
                                            </div>
                                        {/if}

                                        <!-- Selected checkmark -->
                                        {#if selectedSource?.id === source.id && selectedSource?.type === source.type}
                                            <div
                                                class="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-violet-500 flex items-center justify-center shadow-lg shadow-violet-500/30"
                                            >
                                                <svg
                                                    width="11"
                                                    height="11"
                                                    viewBox="0 0 24 24"
                                                    fill="none"
                                                    stroke="white"
                                                    stroke-width="3"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    ><path
                                                        d="M20 6 9 17l-5-5"
                                                    /></svg
                                                >
                                            </div>
                                        {/if}
                                    </div>

                                    <!-- Label -->
                                    <div class="px-2.5 py-2">
                                        <div
                                            class="text-[11.5px] font-medium text-zinc-300 truncate leading-tight"
                                        >
                                            {source.label}
                                        </div>
                                        {#if source.resolution}
                                            <div
                                                class="text-[9.5px] text-zinc-600 mt-0.5 font-mono"
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
                    class="px-5 py-3.5 flex items-center justify-between border-t border-white/[0.05] mt-auto"
                >
                    <button
                        onclick={fetchSources}
                        disabled={isFetching}
                        class="flex items-center gap-1.5 text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors cursor-pointer disabled:opacity-30"
                    >
                        <svg
                            class="w-3.5 h-3.5 {isFetching
                                ? 'animate-spin'
                                : ''}"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            ><path
                                d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"
                            /><path d="M21 3v5h-5" /></svg
                        >
                        Refresh
                    </button>

                    <div class="flex items-center gap-2">
                        <button
                            onclick={closeSelector}
                            class="px-3.5 py-[6px] rounded-lg text-[12px] font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] transition-all cursor-pointer"
                        >
                            Cancel
                        </button>
                        <button
                            onclick={confirmSelection}
                            disabled={!selectedSource}
                            class="px-4 py-[6px] rounded-lg text-[12px] font-semibold bg-white text-zinc-950 hover:bg-zinc-200 transition-all shadow-md cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
                        >
                            Confirm
                        </button>
                    </div>
                </div>
            </div>
        {/if}
    </div>
</main>

<style>
    :global(html),
    :global(body) {
        background: transparent !important;
    }

    /* ══ Pill ══ */
    .island-pill {
        border-radius: 100px;
        background: #0c0c0e;
    }

    /* ══ Expanded ══ */
    .island-expanded {
        border-radius: 20px;
        background: #0c0c0e;
    }

    /* ══ Source card interactions ══ */
    .source-card {
        transition: all 0.2s cubic-bezier(0.22, 1, 0.36, 1);
    }
    .source-card:hover {
        transform: translateY(-1px);
    }
    .source-card:active {
        transform: scale(0.98);
        transition-duration: 0.1s;
    }

    /* ══ Custom scrollbar ══ */
    .custom-scrollbar::-webkit-scrollbar {
        width: 4px;
    }
    .custom-scrollbar::-webkit-scrollbar-track {
        background: transparent;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.06);
        border-radius: 100px;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb:hover {
        background: rgba(255, 255, 255, 0.12);
    }

    /* ══ Entrance animations ══ */
    @keyframes pillFadeIn {
        from {
            opacity: 0;
            transform: translateY(3px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    @keyframes expandFadeIn {
        from {
            opacity: 0;
            transform: translateY(6px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    @keyframes cardPop {
        from {
            opacity: 0;
            transform: scale(0.95);
        }
        to {
            opacity: 1;
            transform: scale(1);
        }
    }
</style>
