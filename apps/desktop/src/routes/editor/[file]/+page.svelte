<script lang="ts">
  import { goto } from "$app/navigation";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import PropertiesPanel from "$components/editor/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPlayerControls from "$components/editor/VideoPlayerControls.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
  import {
    autosaveProject,
    cancelExport,
    clearAutosave,
    createExportId,
    exportVideo,
    generateThumbnails,
    listenToExportState,
    loadEditorDocument,
  } from "$lib/ipc";
  import type { ExportStateEvent } from "$lib/ipc";
  import type { VideoMetadata } from "$lib/stores/editor-store.svelte";
  import { createEditorStore } from "$lib/stores/editor-store.svelte";
  import { ArrowLeft, CheckCircle2, FolderOpen, X } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onDestroy, tick } from "svelte";

  interface Props {
    data: {
      filePath: string;
      filename: string;
    };
  }

  let { data }: Props = $props();

  const store = createEditorStore();

  let videoEl: HTMLVideoElement | null = $state(null);
  let previewContainerEl: HTMLDivElement | null = $state(null);
  let systemAudioEl: HTMLAudioElement | null = $state(null);
  let micAudioEl: HTMLAudioElement | null = $state(null);
  let videoSrc = $state("");
  let systemAudioSrc = $state("");
  let micAudioSrc = $state("");
  let cursorPath = $state<string | null>(null);
  let documentPath = $state("");
  let isLoading = $state(true);
  let error = $state("");
  let loadedPath = $state("");
  let thumbnailToken = 0;

  // Autosave: save edit state every 30 seconds while editing.
  const AUTOSAVE_INTERVAL_MS = 30_000;
  let autosaveTimer: ReturnType<typeof setInterval> | null = null;

  function startAutosave() {
    stopAutosave();
    autosaveTimer = setInterval(async () => {
      if (!documentPath || isLoading) return;
      try {
        const editsJson = JSON.stringify(store.toRenderState());
        await autosaveProject(documentPath, editsJson);
      } catch (err) {
        console.warn("Autosave failed:", err);
      }
    }, AUTOSAVE_INTERVAL_MS);
  }

  function stopAutosave() {
    if (autosaveTimer !== null) {
      clearInterval(autosaveTimer);
      autosaveTimer = null;
    }
  }

  onDestroy(() => {
    stopAutosave();
    // Clear autosave on clean exit.
    if (documentPath) {
      clearAutosave(documentPath).catch(() => {});
    }
  });

  function handleTimeUpdate() {
    if (videoEl && store.isPlaying) {
      store.currentTime = videoEl.currentTime;
      // Cheap drift correction: if audio elements drift > 150ms from video, snap them back.
      const videoT = videoEl.currentTime;
      for (const el of [systemAudioEl, micAudioEl]) {
        if (el && !el.paused && Math.abs(el.currentTime - videoT) > 0.15) {
          el.currentTime = videoT;
        }
      }
    }
  }

  function handleVideoEnded() {
    store.isPlaying = false;
    systemAudioEl?.pause();
    micAudioEl?.pause();
  }

  // Play/pause audio elements in lockstep with the video via the store's
  // `isPlaying` flag (which is set by PlaybackControls, keyboard handler, etc.).
  $effect(() => {
    const playing = store.isPlaying;
    for (const el of [systemAudioEl, micAudioEl]) {
      if (!el) continue;
      if (playing) {
        // Align audio to the video's current time before resuming.
        if (videoEl) el.currentTime = videoEl.currentTime;
        void el.play().catch((err) => {
          console.warn("Audio play failed:", err);
        });
      } else {
        el.pause();
      }
    }
  });

  // Apply volume/mute from the store's audio settings to both audio elements.
  $effect(() => {
    const settings = store.audioSettings;
    const vol = settings.muted ? 0 : Math.max(0, Math.min(1, settings.volume / 100));
    if (systemAudioEl) systemAudioEl.volume = vol;
    if (micAudioEl) micAudioEl.volume = vol;
  });

  // Snap audio to the video's time whenever the user scrubs.
  function handleVideoSeeked() {
    if (!videoEl) return;
    const t = videoEl.currentTime;
    for (const el of [systemAudioEl, micAudioEl]) {
      if (el) el.currentTime = t;
    }
  }

  function mergeVideoMetadata(next: Partial<VideoMetadata>) {
    store.metadata = {
      duration: next.duration ?? store.metadata?.duration ?? 0,
      width: next.width ?? store.metadata?.width ?? 0,
      height: next.height ?? store.metadata?.height ?? 0,
      fps: next.fps ?? store.metadata?.fps ?? 30,
      codec: next.codec ?? store.metadata?.codec ?? "unknown",
      sizeBytes: next.sizeBytes ?? store.metadata?.sizeBytes ?? 0,
    };
    if (store.trimEnd <= 0 && store.metadata.duration > 0) {
      store.loadRenderState({ trimEnd: store.metadata.duration });
    }
  }

  async function loadThumbnailStrip(path: string) {
    const token = ++thumbnailToken;
    try {
      const count =
        store.metadata?.duration && store.metadata.duration > 60 ? 12 : 8;
      const strip = await generateThumbnails(path, count);
      if (token === thumbnailToken) {
        store.thumbnailStrip = strip;
      }
    } catch (err) {
      console.error("Thumbnail generation failed", err);
      if (token === thumbnailToken) {
        store.thumbnailStrip = [];
      }
    }
  }

  function handleVideoLoadedMetadata() {
    if (!videoEl) return;
    mergeVideoMetadata({
      duration: videoEl.duration,
      width: videoEl.videoWidth,
      height: videoEl.videoHeight,
    });
  }

  function handleVideoReady() {
    handleVideoLoadedMetadata();
    isLoading = false;
    startAutosave();
  }

  function handleVideoError() {
    const code = videoEl?.error?.code;
    error = code
      ? `Failed to load source media (media error ${code}).`
      : "Failed to load source media.";
    isLoading = false;
  }

  async function loadDocument() {
    error = "";
    isLoading = true;
    videoSrc = "";
    systemAudioSrc = "";
    micAudioSrc = "";
    cursorPath = null;
    videoEl?.pause();
    systemAudioEl?.pause();
    micAudioEl?.pause();
    store.metadata = null;
    store.reset();
    store.thumbnailStrip = [];

    try {
      const document = await loadEditorDocument(data.filePath);
      documentPath = document.projectPath;
      store.videoPath = document.projectPath;
      store.metadata = document.metadata;
      store.loadRenderState(document.renderState);
      void loadThumbnailStrip(document.projectPath);
      videoSrc = convertFileSrc(document.mediaPath);
      cursorPath = document.cursorPath ?? null;
      store.cursorPath = cursorPath;
      systemAudioSrc = document.audioPath ? convertFileSrc(document.audioPath) : "";
      micAudioSrc = document.microphonePath ? convertFileSrc(document.microphonePath) : "";
      // Mount the editor body so the <video> element exists before we call load().
      // The video element lives inside VideoPreview, which only renders when !isLoading.
      isLoading = false;
      await tick();
      videoEl?.load();
      systemAudioEl?.load();
      micAudioEl?.load();
    } catch (err) {
      console.error("Failed to load editor document", err);
      error = `Could not load project: ${err}`;
      isLoading = false;
    }
  }

  // Export lifecycle UI state — lives in the route, not the store, because the
  // overlay handles success/cancel/error reveals that don't belong in global state.
  let exportStartedAt = $state<number>(0);
  let exportNow = $state<number>(Date.now());
  let exportCancelling = $state(false);
  let exportFinalizing = $state(false);
  let exportHasProgress = $state(false);
  let exportLastProgressAt = $state<number | null>(null);
  let activeExportId = $state<string | null>(null);
  let exportResult = $state<
    | { kind: "success"; path: string }
    | { kind: "cancelled" }
    | { kind: "error"; message: string }
    | null
  >(null);

  function setExportResult(next: NonNullable<typeof exportResult>) {
    let alreadySame = false;
    if (exportResult?.kind === next.kind) {
      if (next.kind === "success" && exportResult.kind === "success") {
        alreadySame = exportResult.path === next.path;
      } else if (next.kind === "error" && exportResult.kind === "error") {
        alreadySame = exportResult.message === next.message;
      } else if (next.kind === "cancelled" && exportResult.kind === "cancelled") {
        alreadySame = true;
      }
    }
    if (alreadySame) return;

    exportResult = next;
    exportFinalizing = false;
    exportCancelling = false;

    if (next.kind === "success") {
      toast.success("Export complete");
    } else if (next.kind === "cancelled") {
      toast.info("Export cancelled");
    } else {
      toast.error("Export failed");
    }
  }

  function handleExportState(event: ExportStateEvent) {
    switch (event.status) {
      case "started":
        return;
      case "progress": {
        const next = Math.min(Math.max(event.progress, 0), 100);
        const current = store.exportProgress ?? 0;

        // FFmpeg progress gets noisy near the end on some Windows builds.
        // Keep the UI monotonic and ignore sub-tenth-percent jitter so the
        // progress bar does not flicker around 99%.
        if (!exportHasProgress || next >= 100 || next > current + 0.1) {
          store.exportProgress = Math.max(current, next);
        }
        exportHasProgress = true;
        exportLastProgressAt = Date.now();
        // Aggressive flip: at ≥99.5% raw progress the encoder has effectively
        // finished — only the mux trailer write remains. Don't wait for the
        // explicit `progress=end` event (which can lag many seconds on Windows
        // due to stderr pipe buffering) before flipping the UI to the
        // indeterminate "Finalizing…" state.
        if (!exportFinalizing && next >= 99.5) {
          exportFinalizing = true;
        }
        return;
      }
      case "finalizing":
        exportFinalizing = true;
        return;
      case "success":
        setExportResult({ kind: "success", path: event.path });
        return;
      case "cancelled":
        setExportResult({ kind: "cancelled" });
        return;
      case "error":
        setExportResult({ kind: "error", message: event.message });
        return;
    }
  }

  async function handleExport() {
    if (store.isExporting) return;
    const exportId = createExportId();
    store.isExporting = true;
    store.exportProgress = 0;
    exportHasProgress = false;
    exportLastProgressAt = null;
    exportCancelling = false;
    exportFinalizing = false;
    activeExportId = exportId;
    exportResult = null;
    exportStartedAt = Date.now();
    exportNow = exportStartedAt;

    const unlistenExportState = await listenToExportState(exportId, handleExportState);
    // Tauri's IPC layer — that round-trip can lag visibly on some systems

    try {
      const path = await exportVideo(
        documentPath || data.filePath,
        store.exportFormat,
        store.exportQuality,
        store.toRenderState(),
        exportId,
      );
      // Safety net: if the export-state success event was missed, fall back to
      // the Promise result. Don't overwrite if the listener already set it.
      if (!exportResult) {
        setExportResult({ kind: "success", path });
      }
    } catch (err) {
      const message = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
      if (!exportResult) {
        if (message.toLowerCase().includes("cancel")) {
          setExportResult({ kind: "cancelled" });
        } else {
          console.error("Export failed:", err);
          setExportResult({ kind: "error", message });
        }
      }
    } finally {
      unlistenExportState();
      if (activeExportId === exportId) {
        activeExportId = null;
      }
      store.isExporting = false;
      store.exportProgress = null;
      exportHasProgress = false;
      exportLastProgressAt = null;
      exportCancelling = false;
      exportFinalizing = false;
    }
  }

  async function handleCancelExport() {
    if (!store.isExporting || exportCancelling || !activeExportId) return;
    exportCancelling = true;
    try {
      await cancelExport(activeExportId);
    } catch (err) {
      toast.error(`Could not cancel: ${err}`);
      exportCancelling = false;
    }
  }

  function dismissExportResult() {
    exportResult = null;
  }

  async function revealExportInFolder() {
    if (exportResult?.kind !== "success") return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_file_location", { path: exportResult.path });
    } catch (err) {
      toast.error(`Could not open folder: ${err}`);
    }
  }

  function formatElapsed(ms: number) {
    const s = Math.floor(ms / 1000);
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  function formatTime(seconds: number) {
    if (!Number.isFinite(seconds) || seconds <= 0) return "0:00.00";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const centiseconds = Math.floor((seconds % 1) * 100);
    return `${mins}:${secs.toString().padStart(2, "0")}.${centiseconds.toString().padStart(2, "0")}`;
  }

  function getExportDuration() {
    const duration = store.metadata?.duration ?? 0;
    const clipEnd = store.trimEnd > 0 ? store.trimEnd : duration;
    return Math.max(0, clipEnd - store.trimStart);
  }

  function getExportRangeLabel() {
    const duration = store.metadata?.duration ?? 0;
    const clipEnd = store.trimEnd > 0 ? store.trimEnd : duration;
    return `${formatTime(store.trimStart)} - ${formatTime(clipEnd)}`;
  }

  function handleBack() {
    goto("/");
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.defaultPrevented) return;

    // Export overlay intercepts Esc before anything else: cancels a running
    // export or dismisses a completed/cancelled/errored result.
    if (e.key === "Escape") {
      if (store.isExporting) {
        e.preventDefault();
        void handleCancelExport();
        return;
      }
      if (exportResult) {
        e.preventDefault();
        dismissExportResult();
        return;
      }
    }

    if (
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement
    ) {
      return;
    }

    switch (e.key) {
      case " ":
        e.preventDefault();
        if (!videoEl) return;
        if (store.isPlaying) {
          videoEl.pause();
          store.isPlaying = false;
        } else {
          videoEl.play();
          store.isPlaying = true;
        }
        break;
      case "ArrowLeft":
        if (videoEl && store.metadata) {
          const frameDur = 1 / (store.metadata.fps || 30);
          videoEl.currentTime = Math.max(0, videoEl.currentTime - frameDur);
          store.currentTime = videoEl.currentTime;
        }
        break;
      case "ArrowRight":
        if (videoEl && store.metadata) {
          const frameDur = 1 / (store.metadata.fps || 30);
          videoEl.currentTime = Math.min(
            store.metadata.duration,
            videoEl.currentTime + frameDur,
          );
          store.currentTime = videoEl.currentTime;
        }
        break;
      case "z":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          if (e.shiftKey) {
            store.redo();
          } else {
            store.undo();
          }
        }
        break;
      case "f":
      case "F":
        if (e.ctrlKey || e.metaKey) return;
        e.preventDefault();
        if (document.fullscreenElement) {
          void document.exitFullscreen();
        } else if (previewContainerEl) {
          void previewContainerEl.requestFullscreen();
        }
        break;
    }
  }

  $effect(() => {
    if (!data.filePath || data.filePath === loadedPath) return;
    loadedPath = data.filePath;
    void loadDocument();
  });

  $effect(() => {
    if (!videoEl) return;
    videoEl.muted = true;
  });

  $effect(() => {
    if (!store.isExporting) return;
    exportNow = Date.now();
    const timer = setInterval(() => {
      const now = Date.now();
      exportNow = now;
      // Near-end stall fallback: the primary flip happens in handleExportState
      // the moment a progress event arrives with pct ≥99.5. This block covers
      // the case where the progress stream dies silently between ~98% and
      // 99.5% — e.g. very short clips where FFmpeg jumps from 98 → progress=end
      // without an intermediate tick at 99.5, and the `progress=end` line
      // itself is buffered. Threshold lowered from 99 → 98 so we catch this.
      if (
        !exportFinalizing
        && exportHasProgress
        && exportLastProgressAt !== null
        && (store.exportProgress ?? 0) >= 98
        && now - exportLastProgressAt > 1500
      ) {
        exportFinalizing = true;
      }
    }, 500);
    return () => clearInterval(timer);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="fixed inset-0 flex min-h-screen w-full flex-col overflow-hidden bg-background text-foreground"
>
  <!-- Dense custom titlebar that embeds the whole editor toolbar in a single row -->
  <CustomTitlebar wrapperClass="h-9">
    <EditorToolbar
      {store}
      filename={data.filename}
      onback={handleBack}
      onexport={handleExport}
    />
  </CustomTitlebar>

  {#if isLoading}
    <EditorSkeleton />
  {:else if error}
    <div class="flex flex-1 items-center justify-center">
      <div class="animate-in fade-in flex max-w-sm flex-col items-center gap-3 text-center duration-500">
        <div class="flex size-10 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive">
          <span class="text-[18px] font-semibold">!</span>
        </div>
        <p class="text-[12px] text-muted-foreground">{error}</p>
        <Button variant="outline" size="sm" onclick={handleBack} class="gap-1.5">
          <ArrowLeft size={13} />
          Back to recordings
        </Button>
      </div>
    </div>
  {:else}
    <div class="flex min-h-0 flex-1 overflow-hidden">
      <!-- Left column: preview + playback + timeline -->
      <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div
          bind:this={previewContainerEl}
          class="flex min-h-0 flex-1 flex-col items-center justify-center bg-muted/10 p-3 pb-4"
        >
          <div class="flex-1 flex min-h-0 w-full items-center justify-center relative">
            <VideoPreview
              {store}
              bind:videoEl
              {videoSrc}
              {cursorPath}
              onTimeUpdate={handleTimeUpdate}
              onEnded={handleVideoEnded}
              onLoadedMetadata={handleVideoLoadedMetadata}
              onReady={handleVideoReady}
              onError={handleVideoError}
              onSeeked={handleVideoSeeked}
            />
          </div>
          <VideoPlayerControls {store} {videoEl} fullscreenTargetEl={previewContainerEl} />
        </div>

        <Timeline {store} {videoEl} />
      </div>

      <!-- Right column: properties panel -->
      <aside class="min-h-0 w-80 shrink-0 border-l border-border xl:w-88">
        <PropertiesPanel {store} />
      </aside>
    </div>
  {/if}

  <!-- Separate audio tracks — .recast projects store system audio and mic audio
       as separate WAVs (the recording.mp4 video stream has no audio). These
       elements are kept in lockstep with the video via $effects above. -->
  {#if systemAudioSrc}
    <!-- svelte-ignore a11y_media_has_caption -->
    <audio
      bind:this={systemAudioEl}
      src={systemAudioSrc}
      preload="auto"
      class="hidden"
    ></audio>
  {/if}
  {#if micAudioSrc}
    <!-- svelte-ignore a11y_media_has_caption -->
    <audio
      bind:this={micAudioEl}
      src={micAudioSrc}
      preload="auto"
      class="hidden"
    ></audio>
  {/if}

  {#if store.isExporting || exportResult}
    <div
      class="animate-in fade-in fixed inset-0 z-50 flex items-center justify-center bg-background/70 backdrop-blur-sm duration-150"
      role="dialog"
      aria-modal="true"
      aria-labelledby="export-dialog-title"
    >
      <div
        class="animate-in zoom-in-95 flex w-full max-w-sm flex-col overflow-hidden rounded-xl border border-border bg-popover shadow-2xl ring-1 ring-border duration-150"
      >
        <!--
          Inner branch keys on `exportResult` (set by the `export-done` event
          from Rust) rather than on `store.isExporting` (which tracks the
          `exportVideo` Promise and can lag visibly while Tauri IPC rounds
          back). This means the UI flips from "Finalizing…" to "Export
          complete" the moment Rust emits `export-done`, even if the Promise
          hasn't resolved yet.
        -->
        {#if !exportResult}
          {@const rawPct = store.exportProgress ?? 0}
          {@const pct = exportFinalizing ? 99.5 : Math.min(Math.max(rawPct, 0), 99.5)}
          {@const isWaiting = !exportHasProgress && !exportFinalizing}
          {@const isIndeterminate = isWaiting || exportFinalizing}
          {@const exportDuration = getExportDuration()}
          {@const exportRange = getExportRangeLabel()}

          <!-- Header: title + live metadata -->
          <header class="flex items-center gap-3 border-b border-border px-4 py-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-primary/20 bg-primary/10"
            >
              <div class="size-3.5 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
            </div>
            <div class="min-w-0 flex-1">
              <h3 id="export-dialog-title" class="text-[13px] font-semibold tracking-tight text-foreground">
                {#if exportCancelling}
                  Cancelling export…
                {:else if exportFinalizing}
                  Writing video file…
                {:else if isWaiting}
                  Preparing export…
                {:else}
                  Exporting video
                {/if}
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                {store.exportFormat.toUpperCase()} · {store.exportQuality.toUpperCase()}
                · {formatTime(exportDuration)} clip
                · {exportRange}
                {#if exportStartedAt}
                  · {formatElapsed(exportNow - exportStartedAt)}
                {/if}
              </p>
            </div>
            <span class="shrink-0 font-mono text-[11px] tabular-nums text-foreground">
              {#if isWaiting}…{:else if exportFinalizing}—{:else}{pct.toFixed(1)}%{/if}
            </span>
          </header>

          <!-- Progress track -->
          <div class="px-4 pt-3">
            <div class="relative h-1 overflow-hidden rounded-full bg-muted">
              {#if isIndeterminate}
                <div
                  class="animate-recast-indeterminate absolute inset-y-0 left-0 w-1/3 rounded-full bg-primary"
                ></div>
              {:else}
                <div
                  class="h-full rounded-full bg-primary transition-[width] duration-300"
                  style="width: {Math.min(100, Math.max(0, pct))}%"
                ></div>
              {/if}
            </div>
          </div>

          <!-- Footer: kbd hints + cancel -->
          <footer
            class="mt-3 flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <span class="flex items-center gap-1">
              <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
              <span>Cancel</span>
            </span>
            <Button
              variant="destructive_soft"
              size="xs"
              class="gap-1.5"
              onclick={handleCancelExport}
              disabled={exportCancelling}
            >
              <X size={11} />
              {exportCancelling ? "Cancelling…" : "Cancel export"}
            </Button>
          </footer>
        {:else if exportResult?.kind === "success"}
          <header class="flex items-center gap-3 border-b border-border px-4 py-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-success/20 bg-success/10 text-success"
            >
              <CheckCircle2 size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3 id="export-dialog-title" class="text-[13px] font-semibold tracking-tight text-foreground">
                Export complete
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                {store.exportFormat.toUpperCase()} · {store.exportQuality.toUpperCase()}
              </p>
            </div>
          </header>
          <div class="px-4 py-3">
            <p class="truncate font-mono text-[10px] text-muted-foreground" title={exportResult.path}>
              {exportResult.path}
            </p>
          </div>
          <footer
            class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <span class="flex items-center gap-1">
              <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
              <span>Dismiss</span>
            </span>
            <div class="flex items-center gap-1.5">
              <Button variant="ghost" size="xs" onclick={dismissExportResult}>Dismiss</Button>
              <Button variant="default" size="xs" class="gap-1.5" onclick={revealExportInFolder}>
                <FolderOpen size={11} />
                Show in Folder
              </Button>
            </div>
          </footer>
        {:else if exportResult?.kind === "cancelled"}
          <header class="flex items-center gap-3 border-b border-border px-4 py-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-muted text-muted-foreground"
            >
              <X size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3 id="export-dialog-title" class="text-[13px] font-semibold tracking-tight text-foreground">
                Export cancelled
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                No file was written.
              </p>
            </div>
          </header>
          <footer
            class="flex h-10 items-center justify-end gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <Button variant="ghost" size="xs" onclick={dismissExportResult}>Dismiss</Button>
            <Button variant="default" size="xs" onclick={handleExport}>Export again</Button>
          </footer>
        {:else if exportResult?.kind === "error"}
          <header class="flex items-center gap-3 border-b border-border px-4 py-3">
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive"
            >
              <X size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3 id="export-dialog-title" class="text-[13px] font-semibold tracking-tight text-foreground">
                Export failed
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">Something went wrong.</p>
            </div>
          </header>
          <div class="max-h-40 overflow-y-auto border-b border-border px-4 py-3">
            <pre class="whitespace-pre-wrap wrap-break-word font-mono text-[10px] text-destructive">{exportResult.message}</pre>
          </div>
          <footer
            class="flex h-10 items-center justify-end gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <Button variant="ghost" size="xs" onclick={dismissExportResult}>Dismiss</Button>
            <Button variant="default" size="xs" onclick={handleExport}>Try again</Button>
          </footer>
        {/if}
      </div>
    </div>
  {/if}
</div>
