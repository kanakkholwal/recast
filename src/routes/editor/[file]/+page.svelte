<script lang="ts">
  import { goto } from "$app/navigation";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import PlaybackControls from "$components/editor/PlaybackControls.svelte";
  import PropertiesPanel from "$components/editor/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
  import { Button } from "$components/ui/button";
  import {
    autosaveProject,
    clearAutosave,
    exportVideo,
    generateThumbnails,
    loadEditorDocument,
  } from "$lib/ipc";
  import type { VideoMetadata } from "$lib/stores/editor-store.svelte";
  import { createEditorStore } from "$lib/stores/editor-store.svelte";
  import { ArrowLeft } from "@lucide/svelte";
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
            onSeeked={handleVideoSeeked}
          />
        </div>

        <PlaybackControls {store} {videoEl} />
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

  {#if store.isExporting}
    <div
      class="animate-in fade-in fixed inset-0 z-50 flex items-center justify-center bg-background/70 backdrop-blur-sm duration-200"
    >
      <div
        class="animate-in zoom-in-95 flex min-w-70 flex-col gap-3 rounded-xl border border-border bg-popover p-5 shadow-2xl ring-1 ring-border duration-200"
      >
        <div class="flex items-center justify-center gap-3">
          <div class="size-4 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
          <div class="flex-1">
            <p class="text-sm font-semibold text-foreground">Exporting video</p>
            <p class="text-xs text-muted-foreground">
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
