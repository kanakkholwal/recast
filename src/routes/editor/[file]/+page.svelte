<script lang="ts">
  import { goto } from "$app/navigation";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import PlaybackControls from "$components/editor/PlaybackControls.svelte";
  import PropertiesPanel from "$components/editor/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
  import {
    autosaveProject,
    clearAutosave,
    exportVideo,
    generateThumbnails,
    loadEditorDocument,
  } from "$lib/ipc";
  import type { VideoMetadata } from "$lib/stores/editor-store.svelte";
  import { createEditorStore } from "$lib/stores/editor-store.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
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
  let videoSrc = $state("");
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
    }
  }

  function handleVideoEnded() {
    store.isPlaying = false;
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
    cursorPath = null;
    videoEl?.pause();
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
      // Mount the editor body so the <video> element exists before we call load().
      // The video element lives inside VideoPreview, which only renders when !isLoading.
      isLoading = false;
      await tick();
      videoEl?.load();
    } catch (err) {
      console.error("Failed to load editor document", err);
      error = `Could not load project: ${err}`;
      isLoading = false;
    }
  }

  async function handleExport() {
    if (store.isExporting) return;
    store.isExporting = true;
    store.exportProgress = 0;

    const unlisten = await listen<number>("export-progress", (event) => {
      store.exportProgress = event.payload;
    });

    try {
      await exportVideo(
        documentPath || data.filePath,
        store.exportFormat,
        store.exportQuality,
        store.toRenderState(),
      );
    } catch (err) {
      console.error("Export failed:", err);
      alert(`Export failed: ${err}`);
    } finally {
      unlisten();
      store.isExporting = false;
      store.exportProgress = null;
    }
  }

  function handleBack() {
    goto("/");
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
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
        <button
          onclick={handleBack}
          class="rounded-md border border-border bg-background px-3 py-1.5 text-[11px] font-medium text-foreground transition-colors hover:bg-muted"
        >
          ← Back to recordings
        </button>
      </div>
    </div>
  {:else}
    <div class="flex min-h-0 flex-1 overflow-hidden">
      <!-- Left column: preview + playback + timeline -->
      <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div class="flex min-h-0 flex-1 items-center justify-center bg-muted/10 p-3">
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
          />
        </div>

        <PlaybackControls {store} {videoEl} />
        <Timeline {store} {videoEl} />
      </div>

      <!-- Right column: properties panel -->
      <aside class="min-h-0 w-80 shrink-0 border-l border-border xl:w-[22rem]">
        <PropertiesPanel {store} />
      </aside>
    </div>
  {/if}

  {#if store.isExporting}
    <div
      class="animate-in fade-in fixed inset-0 z-50 flex items-center justify-center bg-background/70 backdrop-blur-sm duration-200"
    >
      <div
        class="animate-in zoom-in-95 flex min-w-[280px] flex-col gap-3 rounded-xl border border-border bg-popover p-5 shadow-2xl ring-1 ring-border duration-200"
      >
        <div class="flex items-center gap-3">
          <div class="size-4 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
          <div class="flex-1">
            <p class="text-[12px] font-semibold text-foreground">Exporting video</p>
            <p class="text-[11px] text-muted-foreground">
              {store.exportFormat.toUpperCase()} ·
              {store.exportProgress !== null ? `${Math.round(store.exportProgress)}%` : "Preparing…"}
            </p>
          </div>
        </div>
        {#if store.exportProgress !== null}
          <div class="h-1 overflow-hidden rounded-full bg-muted">
            <div
              class="h-full rounded-full bg-primary transition-[width] duration-300"
              style="width: {store.exportProgress}%"
            ></div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
