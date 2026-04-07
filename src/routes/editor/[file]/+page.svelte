<script lang="ts">
  import { goto } from "$app/navigation";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import PlaybackControls from "$components/editor/PlaybackControls.svelte";
  import PropertiesPanel from "$components/editor/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import { Spinner } from "$components/ui/spinner";
  import type {
    EditorRenderState,
    VideoMetadata,
  } from "$lib/stores/editor-store.svelte";
  import { createEditorStore } from "$lib/stores/editor-store.svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";

  interface Props {
    data: {
      filePath: string;
      filename: string;
    };
  }

  interface EditorDocument {
    projectPath: string;
    mediaPath: string;
    cursorPath?: string | null;
    editsPath?: string | null;
    metadata: VideoMetadata;
    renderState: EditorRenderState;
  }

  let { data }: Props = $props();

  const store = createEditorStore();

  let videoEl: HTMLVideoElement | null = $state(null);
  let videoSrc = $state("");
  let documentPath = $state("");
  let previewSrc = $state("");
  let previewFallbackSrc = $state("");
  let isLoading = $state(true);
  let isRenderingPreview = $state(false);
  let error = $state("");
  let loadedPath = $state("");
  let loadStage = $state("Opening project");
  let previewToken = 0;
  let thumbnailToken = 0;
  let lastPreviewKey = "";

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

  async function renderPreview(force = false) {
    if (!documentPath) return;
    const previewTime = store.isPlaying
      ? Math.round(store.currentTime * 8) / 8
      : store.currentTime;
    const renderState = store.toRenderState();
    const previewKey = `${documentPath}|${previewTime.toFixed(3)}|${JSON.stringify(renderState)}`;
    if (!force && previewKey === lastPreviewKey) return;

    if (isRenderingPreview && !force) return;

    lastPreviewKey = previewKey;
    const token = ++previewToken;
    isRenderingPreview = true;

    try {
      const frame = await invoke<string>("render_preview_frame", {
        request: {
          inputPath: documentPath,
          time: previewTime,
          renderState,
        },
      });
      if (token === previewToken) {
        previewSrc = frame;
      }
    } catch (err) {
      console.error("Preview render failed", err);
      if (token === previewToken) {
        previewSrc = "";
      }
    } finally {
      if (token === previewToken) {
        isRenderingPreview = false;
      }
    }
  }

  async function loadThumbnailStrip(path: string) {
    const token = ++thumbnailToken;
    try {
      const count =
        store.metadata?.duration && store.metadata.duration > 60 ? 12 : 8;
      const strip = await invoke<string[]>("generate_thumbnails", {
        path,
        count,
      });
      if (token === thumbnailToken) {
        store.thumbnailStrip = strip;
        previewFallbackSrc = strip[0] ?? "";
      }
    } catch (err) {
      console.error("Thumbnail generation failed", err);
      if (token === thumbnailToken) {
        store.thumbnailStrip = [];
        previewFallbackSrc = "";
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
    loadStage = "Ready";
    isLoading = false;
    void renderPreview(true);
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
    loadStage = "Opening project";
    previewSrc = "";
    previewFallbackSrc = "";
    videoSrc = "";
    videoEl?.pause();
    store.metadata = null;
    store.reset();
    store.thumbnailStrip = [];

    try {
      loadStage = "Reading recording metadata";
      const document = await invoke<EditorDocument>("load_editor_document", {
        path: data.filePath,
      });

      documentPath = document.projectPath;
      store.videoPath = document.projectPath;
      store.metadata = document.metadata;
      store.loadRenderState(document.renderState);
      loadStage = "Building timeline";
      void loadThumbnailStrip(document.projectPath);

      videoSrc = convertFileSrc(document.mediaPath);
      loadStage = "Rendering preview";
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

    try {
      const result = await invoke<string>("export_video", {
        request: {
          inputPath: documentPath || data.filePath,
          format: store.exportFormat,
          quality: store.exportQuality,
          renderState: store.toRenderState(),
        },
      });
      console.log("Export complete:", result);
    } catch (err) {
      console.error("Export failed:", err);
      alert(`Export failed: ${err}`);
    } finally {
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
    if (!documentPath || isLoading || error) return;
    void renderPreview();
  });

  $effect(() => {
    if (!videoEl) return;
    videoEl.muted = true;
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="flex min-h-screen w-full flex-col overflow-hidden bg-background text-foreground fixed inset-0"
>
  <EditorToolbar
    {store}
    filename={data.filename}
    onback={handleBack}
    onexport={handleExport}
  />

  <div class="flex min-h-0 flex-1 overflow-hidden">
    <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div class="flex min-h-0 flex-1 items-center justify-center p-4 pb-2">
        {#if isLoading}
          <div
            class="animate-in fade-in flex w-full max-w-lg flex-col items-center gap-5 duration-500"
          >
            <div
              class="relative flex h-20 w-20 items-center justify-center rounded-[24px] border border-border/70 bg-card shadow-lg"
            >
              <div
                class="absolute inset-2 rounded-[18px] bg-linear-to-br from-primary/15 via-primary/5 to-transparent"
              ></div>
              <div
                class="h-9 w-9 animate-spin rounded-full border-2 border-primary border-t-transparent"
              ></div>
            </div>
            <div class="space-y-2 text-center">
              <p class="text-base font-semibold text-foreground">
                Preparing editor
              </p>
              <p class="text-sm text-muted-foreground">{loadStage}...</p>
            </div>
            <div
              class="w-full rounded-full border border-border/60 bg-card/80 p-1"
            >
              <div class="h-2 w-full overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full w-2/3 animate-pulse rounded-full bg-linear-to-r from-primary via-sky-400 to-primary"
                ></div>
              </div>
            </div>
          </div>
        {:else if error}
          <div
            class="animate-in fade-in flex flex-col items-center gap-4 text-center duration-500"
          >
            <div
              class="flex h-16 w-16 items-center justify-center rounded-2xl bg-destructive/10 text-destructive"
            >
              <span class="text-2xl">!</span>
            </div>
            <p class="max-w-sm text-sm text-muted-foreground">{error}</p>
            <button
              onclick={handleBack}
              class="text-sm text-primary hover:underline"
            >
              Back to recordings
            </button>
          </div>
        {:else}
          <VideoPreview
            {store}
            {previewSrc}
            fallbackSrc={previewFallbackSrc}
            isRendering={isRenderingPreview}
          />
        {/if}
      </div>

      <PlaybackControls {store} {videoEl} />
      <Timeline {store} {videoEl} />
    </div>

    <div class="min-h-0 w-85 shrink-0 xl:w-90 border-l">
      {#if isLoading}
      <div class="inline-flex justify-center items-center gap-2 h-96 w-full">
		  <Spinner class="size-6" />
	  </div>
      {:else}
        <PropertiesPanel {store} />
      {/if}
    </div>
  </div>

  {#if videoSrc}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={videoEl}
      src={videoSrc}
      ontimeupdate={handleTimeUpdate}
      onended={handleVideoEnded}
      onloadedmetadata={handleVideoLoadedMetadata}
      onloadeddata={handleVideoReady}
      oncanplay={handleVideoReady}
      onerror={handleVideoError}
      class="pointer-events-none absolute -z-10 opacity-0"
      playsinline
      preload="auto"
    ></video>
  {/if}

  {#if store.isExporting}
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm animate-in fade-in duration-200"
    >
      <div
        class="animate-in zoom-in-95 flex flex-col items-center gap-4 rounded-2xl border border-border bg-card p-8 shadow-2xl duration-300"
      >
        <div
          class="h-10 w-10 animate-spin rounded-full border-3 border-primary border-t-transparent"
        ></div>
        <div class="text-center">
          <p class="text-sm font-semibold text-foreground">
            Exporting video...
          </p>
          <p class="mt-1 text-xs text-muted-foreground">
            {store.exportFormat.toUpperCase()} - {store.exportProgress !== null
              ? `${Math.round(store.exportProgress)}%`
              : "Preparing..."}
          </p>
        </div>
        {#if store.exportProgress !== null}
          <div class="h-1.5 w-48 overflow-hidden rounded-full bg-muted">
            <div
              class="h-full rounded-full bg-linear-to-r from-primary to-blue-400 transition-[width] duration-300"
              style="width: {store.exportProgress}%"
            ></div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
