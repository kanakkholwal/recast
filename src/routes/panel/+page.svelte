<script lang="ts">
    import { getDisplays, startRecording, stopRecording } from "$lib/ipc";
    import { emit, listen } from "@tauri-apps/api/event";
    import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { onMount } from "svelte";

    type TargetSource = {
        type: "monitor" | "window";
        id: number;
        label: string;
    };

    let selectedSource: TargetSource | null = $state(null);
    let isRecording = $state(false);
    let recordingStartTime: number | null = $state(null);
    let now = $state(Date.now());
    let timerInterval: number;

    onMount(() => {
        timerInterval = window.setInterval(() => {
            if (isRecording) now = Date.now();
        }, 1000);

        const unlisten = listen<TargetSource>("source-selected", (event) => {
            selectedSource = event.payload;
        });

        getDisplays()
            .then((displays) => {
                if (displays.length > 0 && !selectedSource) {
                    const d = displays[0];
                    selectedSource = {
                        type: "monitor",
                        id: d.id,
                        label: d.isPrimary ? "Primary Display" : `Display 1`,
                    };
                }
            })
            .catch(() => {});

        return () => {
            window.clearInterval(timerInterval);
            unlisten.then((fn) => fn());
        };
    });

    async function openSourceSelector() {
        if (isRecording) return;
        const existing = await WebviewWindow.getByLabel("source-selector");
        if (existing) {
            await existing.setFocus();
            return;
        }
        const selectorWin = new WebviewWindow("source-selector", {
            url: "/select",
            title: "Select Source — Recast",
            width: 660,
            height: 500,
            center: true,
            decorations: false,
            resizable: false,
        });
        selectorWin.once("tauri://error", (e) => console.error(e));
    }

    async function closeApp() {
        await getCurrentWindow().close();
    }

    async function toggleRecording() {
        if (isRecording) {
            try {
                const filePath = await stopRecording();
                isRecording = false;
                recordingStartTime = null;
                // toast.success("Recording saved successfully!", {
                //     description: filePath,
                // });
                console.log("Saved:", filePath);
                // Also trigger main window to refresh recordings if it's open
                await emit("refresh-recordings");
            } catch (e) {
                alert(`Stop failed: ${e}\n\nMake sure ffmpeg is installed.`);
            }
        } else {
            if (!selectedSource) return;
            try {
                await startRecording(selectedSource.type, selectedSource.id);
                isRecording = true;
                now = Date.now();
                recordingStartTime = now;
            } catch (e) {
                alert(
                    `Start failed: ${e}\n\nMake sure ffmpeg is installed and available in PATH.`,
                );
            }
        }
    }

    let elapsed = $derived(
        isRecording && recordingStartTime
            ? Math.floor((now - recordingStartTime) / 1000)
            : 0,
    );
    let timer = $derived(
        `${Math.floor(elapsed / 60)
            .toString()
            .padStart(2, "0")}:${(elapsed % 60).toString().padStart(2, "0")}`,
    );
</script>

<div
    class="w-full h-screen rounded-[12px] flex items-center gap-2 px-3 bg-card border text-foreground font-sans shadow-lg overflow-hidden"
    data-tauri-drag-region
>
    <!-- Window controls (just close the panel) -->
    <div class="flex items-center gap-1.5 shrink-0">
        <button
            onclick={closeApp}
            class="w-3.5 h-3.5 rounded-full bg-red-500 opacity-75 hover:opacity-100 transition-opacity"
            title="Close Panel"
            aria-label="Close"
        ></button>
    </div>

    <!-- Divider -->
    <div class="w-px h-5 bg-border/80 shrink-0 mx-0.5"></div>

    <!-- Status + Source -->
    <div class="flex items-center gap-2 flex-1 min-w-0" data-tauri-drag-region>
        <div
            class="w-2 h-2 rounded-full shrink-0 {isRecording
                ? 'bg-red-500 shadow-[0_0_8px_rgba(239,68,68,0.6)] animate-pulse'
                : 'bg-emerald-500 shadow-[0_0_6px_rgba(52,211,153,0.4)]'}"
        ></div>
        <button
            disabled={isRecording}
            onclick={openSourceSelector}
            class="truncate text-left text-[13px] font-medium transition-opacity hover:opacity-70 disabled:opacity-40 min-w-0"
        >
            {selectedSource?.label ?? "Select Source"}
        </button>
    </div>

    <!-- Timer -->
    <span
        class="font-mono text-[12px] tabular-nums tracking-widest text-neutral-500 dark:text-neutral-400 shrink-0 w-11 text-right"
        data-tauri-drag-region
    >
        {timer}
    </span>

    <!-- Divider -->
    <div class="w-px h-5 bg-black/10 dark:bg-white/10 shrink-0 mx-1"></div>

    <!-- Record -->
    <button
        onclick={toggleRecording}
        class="w-8 h-8 rounded-full flex items-center justify-center border border-black/10 dark:border-white/10 bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 transition-colors shrink-0 {isRecording
            ? 'bg-red-500/10 border-red-500/30 hover:bg-red-500/20'
            : ''}"
        title={isRecording ? "Stop" : "Record"}
        aria-label={isRecording ? "Stop" : "Record"}
    >
        {#if isRecording}
            <div
                class="w-2.5 h-2.5 rounded-[2px] bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.4)]"
            ></div>
        {:else}
            <div
                class="w-3.5 h-3.5 rounded-full bg-linear-to-br from-violet-500 to-fuchsia-500 shadow-[0_0_8px_rgba(139,92,246,0.3)]"
            ></div>
        {/if}
    </button>
</div>
