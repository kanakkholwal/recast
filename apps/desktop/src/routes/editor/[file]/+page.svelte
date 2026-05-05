<script lang="ts">
  import { goto } from "$app/navigation";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import PropertiesPanel from "$components/editor/properity-panel/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPlayerControls from "$components/editor/VideoPlayerControls.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
  import { rasterizeCursorSprites } from "$lib/export/rasterize-cursor";
  import { expandTextAnnotations } from "$lib/export/rasterize-text";
  import type { ExportStateEvent } from "$lib/ipc";
  import {
    autosaveProject,
    cancelExport,
    clearAutosave,
    createExportId,
    exportVideo,
    generateThumbnails,
    listenToExportState,
    loadEditorDocument,
    saveProjectEdits,
    suggestZoomRegions,
  } from "$lib/ipc";
  import {
    createEditorStore,
    framePaddingPixels,
    type VideoMetadata,
  } from "$lib/stores/editor-store.svelte";
  import { applyAutoZooms } from "$lib/zoom/auto-apply";
  import { ArrowLeft, CheckCircle2, FolderOpen, X } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
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
    const vol = settings.muted
      ? 0
      : Math.max(0, Math.min(1, settings.volume / 100));
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
      systemAudioSrc = document.audioPath
        ? convertFileSrc(document.audioPath)
        : "";
      micAudioSrc = document.microphonePath
        ? convertFileSrc(document.microphonePath)
        : "";
      // Mount the editor body so the <video> element exists before we call load().
      // The video element lives inside VideoPreview, which only renders when !isLoading.
      isLoading = false;
      await tick();
      videoEl?.load();
      systemAudioEl?.load();
      micAudioEl?.load();
      void maybeRunAutoZoom();
    } catch (err) {
      console.error("Failed to load editor document", err);
      error = `Could not load project: ${err}`;
      isLoading = false;
    }
  }

  // Smart Auto-Zoom: on the first load of a recording, place a focus region
  // at every detected click + settle-after-motion. Persisted via the
  // `autoZoomApplied` flag on the project document so subsequent reopens
  // don't repopulate (the user may have intentionally cleared regions).
  let autoZoomRunning = false;

  async function maybeRunAutoZoom() {
    if (autoZoomRunning) return;
    if (!store.autoZoomEnabled || store.autoZoomApplied) return;
    if (!cursorPath) {
      // Screen-only recording with no cursor track to analyse — latch the
      // flag so we don't retry every reopen.
      store.autoZoomApplied = true;
      return;
    }
    if (store.zoomRegions.length > 0) {
      // Project already has regions (autosave restored them, or the user
      // added some manually before the auto-apply ran). Skip silently.
      store.autoZoomApplied = true;
      return;
    }
    await runAutoZoom({ silentEmpty: true });
  }

  async function runAutoZoom(opts: { silentEmpty?: boolean } = {}) {
    if (autoZoomRunning) return;
    if (!cursorPath) return;
    autoZoomRunning = true;
    try {
      const suggestions = await suggestZoomRegions(cursorPath);
      const dur = store.metadata?.duration ?? 0;
      const w = store.metadata?.width ?? 0;
      const h = store.metadata?.height ?? 0;
      const bounds = {
        start: store.inPoint,
        end: store.outPoint > 0 ? store.outPoint : dur,
      };
      if (bounds.end <= bounds.start) {
        store.autoZoomApplied = true;
        return;
      }
      // Single coalesced undo entry covering all auto-applied regions.
      store.pushUndoState();
      const result = applyAutoZooms(store, suggestions, bounds, w, h);
      store.autoZoomApplied = true;
      // Persist immediately so a crash before the 30 s autosave tick doesn't
      // re-run auto-zoom on next open and double up regions.
      if (documentPath) {
        try {
          await autosaveProject(
            documentPath,
            JSON.stringify(store.toRenderState()),
          );
        } catch (err) {
          console.warn("Auto-zoom autosave failed:", err);
        }
      }
      if (result.applied > 0) {
        toast.success(
          `Added ${result.applied} focus moment${result.applied === 1 ? "" : "s"}`,
          {
            description: "Tweak, remove, or turn off in the Focus panel.",
            action: {
              label: "Undo",
              onClick: () => {
                store.clearAutoZooms();
                store.autoZoomApplied = false;
              },
            },
          },
        );
      } else if (!opts.silentEmpty) {
        toast.info("No focus candidates found");
      }
    } catch (err) {
      console.warn("Auto-zoom failed:", err);
    } finally {
      autoZoomRunning = false;
    }
  }

  // Re-run is exposed to FocusPanel via a typed CustomEvent on `window` so
  // the deeply-nested panel doesn't need to thread a prop through every
  // intermediate component.
  $effect(() => {
    function onRerun() {
      store.clearAutoZooms();
      store.autoZoomApplied = false;
      void runAutoZoom({ silentEmpty: false });
    }
    window.addEventListener("recast:rerun-auto-zoom", onRerun);
    return () => window.removeEventListener("recast:rerun-auto-zoom", onRerun);
  });

  // Export lifecycle UI state — lives in the route, not the store, because the
  // overlay handles success/cancel/error reveals that don't belong in global state.
  let exportStartedAt = $state<number>(0);
  let exportNow = $state<number>(Date.now());
  let exportCancelling = $state(false);
  let exportFinalizing = $state(false);
  let exportHasProgress = $state(false);
  let activeExportId = $state<string | null>(null);

  // Preparing-stage substages — surfaced in the dialog so users see the
  // hybrid-raster work happening rather than a generic spinner.
  let prepText = $state<"pending" | "running" | "done">("pending");
  let prepCursor = $state<"pending" | "running" | "done">("pending");
  let prepSending = $state<"pending" | "running" | "done">("pending");
  function resetPrep() {
    prepText = "pending";
    prepCursor = "pending";
    prepSending = "pending";
  }

  // Eased display percentage. Raw FFmpeg progress is jumpy (5–10% jumps,
  // sticky around 99%), so we lerp the rendered ring toward it with a
  // cubic-bezier-like response. Rerun on every animation tick while
  // exporting so the ring never sits stale.
  let displayPct = $state(0);
  let easeRafHandle: number | null = null;
  $effect(() => {
    if (!store.isExporting) {
      if (easeRafHandle !== null) {
        cancelAnimationFrame(easeRafHandle);
        easeRafHandle = null;
      }
      displayPct = 0;
      return;
    }
    let lastTs: number | null = null;
    function tick(now: number) {
      const target = exportFinalizing
        ? 99.5
        : Math.min(99.5, Math.max(0, store.exportProgress ?? 0));
      const dt = lastTs === null ? 16 : Math.max(1, Math.min(64, now - lastTs));
      lastTs = now;
      // Critically-damped follower with a ~250 ms time constant. The
      // exponential form is shape-equivalent to a cubic-bezier-ease-out
      // toward `target` and avoids overshoot at the high end.
      const tau = 250;
      const k = 1 - Math.exp(-dt / tau);
      const next = displayPct + (target - displayPct) * k;
      // Don't animate backwards on micro-jitter; the underlying export is
      // monotonic so the ring should feel monotonic too.
      displayPct = Math.max(displayPct, next);
      easeRafHandle = requestAnimationFrame(tick);
    }
    easeRafHandle = requestAnimationFrame(tick);
    return () => {
      if (easeRafHandle !== null) {
        cancelAnimationFrame(easeRafHandle);
        easeRafHandle = null;
      }
    };
  });

  function renderStateHasText(): boolean {
    return store.annotations.some((a) => a.kind.kind === "text");
  }

  // ETA — only meaningful once we have ≥10% real progress. Computed from
  // wall-clock-elapsed × (1 − pct) / pct, smoothed by the same ring follower.
  function exportEtaMs(): number | null {
    if (!exportHasProgress || exportFinalizing) return null;
    const pct = store.exportProgress ?? 0;
    if (pct < 10) return null;
    const elapsed = exportNow - exportStartedAt;
    if (elapsed < 250) return null;
    return (elapsed * (100 - pct)) / pct;
  }

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
      } else if (
        next.kind === "cancelled" &&
        exportResult.kind === "cancelled"
      ) {
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
        // Previously this block speculatively flipped the UI to "finalizing"
        // at ≥99.5% raw progress, on the assumption that stderr pipe batching
        // was hiding the real `progress=end`. With `-progress pipe:2
        // -stats_period 0.1` the Rust side now emits a real `finalizing`
        // event within ~100ms of ffmpeg's actual finish, so the speculative
        // flip was just mislabelling the last second of active encoding as
        // "Writing video file…". Only a very-near-end safety net remains
        // below as a last-resort for the rare case where the `finalizing`
        // event is dropped entirely.
        if (!exportFinalizing && next >= 99.95) {
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
    exportCancelling = false;
    exportFinalizing = false;
    activeExportId = exportId;
    exportResult = null;
    exportStartedAt = Date.now();
    exportNow = exportStartedAt;
    resetPrep();

    const unlistenExportState = await listenToExportState(
      exportId,
      handleExportState,
    );
    // Tauri's IPC layer — that round-trip can lag visibly on some systems

    try {
      // Hybrid-raster pass: replace text annotations with image-kind ones
      // whose `path` is a base64-encoded PNG. Rust's draw_image consumes
      // both file paths and `data:` URLs uniformly.
      const renderState = store.toRenderState();
      const meta = store.metadata;
      const paddingPx = framePaddingPixels(renderState.padding ?? 0, meta);
      const canvasW = meta ? meta.width + paddingPx * 2 : 0;
      const canvasH = meta ? meta.height + paddingPx * 2 : 0;
      // Run the two hybrid-raster passes in parallel — they don't depend
      // on each other and the cursor SVG decode is non-trivial on cold
      // boot (Image() onload is async even for inline blobs). This trims
      // perceived "Preparing…" time roughly in half on projects with text.
      prepText = renderState.annotations.some((a) => a.kind.kind === "text")
        ? "running"
        : "done";
      prepCursor = store.cursorSettings.style !== "dot" ? "running" : "done";
      const [expandedAnnotations, cursorSprites] = await Promise.all([
        expandTextAnnotations(renderState.annotations, canvasW, canvasH).then(
          (r) => {
            prepText = "done";
            return r;
          },
        ),
        rasterizeCursorSprites(
          store.cursorSettings.style,
          store.cursorSettings.size * 16,
        ).then((r) => {
          prepCursor = "done";
          return r;
        }),
      ]);
      prepSending = "running";
      const finalRenderState = {
        ...renderState,
        annotations: expandedAnnotations,
        cursorSpriteRest: cursorSprites?.rest,
        cursorSpritePress: cursorSprites?.press,
        cursorSpriteHotspotRest: cursorSprites?.restHotspot,
        cursorSpriteHotspotPress: cursorSprites?.pressHotspot,
        cursorSpriteSizePx: cursorSprites?.pixelSize,
      };

      prepSending = "done";
      const path = await exportVideo(
        documentPath || data.filePath,
        store.exportFormat,
        store.exportQuality,
        finalRenderState,
        exportId,
        store.exportFormat === "gif" ? store.gifSettings : undefined,
      );
      // Safety net: if the export-state success event was missed, fall back to
      // the Promise result. Don't overwrite if the listener already set it.
      if (!exportResult) {
        setExportResult({ kind: "success", path });
      }
    } catch (err) {
      const message =
        typeof err === "string"
          ? err
          : err instanceof Error
            ? err.message
            : String(err);
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

  let isSaving = $state(false);

  async function handleSave() {
    if (!documentPath || isSaving || isLoading) return;
    isSaving = true;
    try {
      const editsJson = JSON.stringify(store.toRenderState());
      const savedAt = await saveProjectEdits(documentPath, editsJson);
      store.markSaved(savedAt);
      toast.success("Saved");
    } catch (err) {
      const message =
        typeof err === "string"
          ? err
          : err instanceof Error
            ? err.message
            : String(err);
      toast.error(`Couldn't save: ${message}`);
    } finally {
      isSaving = false;
    }
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
      case "s":
      case "S":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          void handleSave();
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
    // Elapsed-time timer for the export status strip. The Rust side is now
    // the source of truth for when the UI flips to "finalizing" — it emits
    // an explicit event on ffmpeg's `progress=end`, typically within ~100ms.
    // No more client-side speculative flips on progress stalls.
    const timer = setInterval(() => {
      exportNow = Date.now();
    }, 500);
    return () => clearInterval(timer);
  });

  const stages = $derived([
    {
      key: "text" as const,
      label: "Render text overlays",
      state: prepText,
      skip: prepText === "done" && !renderStateHasText(),
    },
    {
      key: "cursor" as const,
      label: "Render cursor sprites",
      state: prepCursor,
      skip: prepCursor === "done" && store.cursorSettings.style === "dot",
    },
    {
      key: "ship" as const,
      label: "Hand off to encoder",
      state: prepSending,
    },
    {
      key: "encode" as const,
      label: exportFinalizing ? "Finalise file" : "Encode frames",
      state:
        prepSending !== "done"
          ? "pending"
          : exportFinalizing
            ? "running"
            : exportHasProgress
              ? "running"
              : "pending",
    },
  ]);
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
      onsave={handleSave}
      {isSaving}
    />
  </CustomTitlebar>

  {#if isLoading}
    <EditorSkeleton />
  {:else if error}
    <div class="flex flex-1 items-center justify-center">
      <div
        class="animate-in fade-in flex max-w-sm flex-col items-center gap-3 text-center duration-500"
      >
        <div
          class="flex size-10 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive"
        >
          <span class="text-[18px] font-semibold">!</span>
        </div>
        <p class="text-[12px] text-muted-foreground">{error}</p>
        <Button
          variant="outline"
          size="sm"
          onclick={handleBack}
          class="gap-1.5"
        >
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
          class="flex min-h-0 flex-1 flex-col items-center justify-center bg-background px-2 pt-1.5 pb-1"
        >
          <div
            class="flex-1 flex min-h-0 w-full items-center justify-center relative"
          >
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
          <VideoPlayerControls
            {store}
            {videoEl}
            fullscreenTargetEl={previewContainerEl}
          />
        </div>

        <Timeline {store} {videoEl} />
      </div>

      <!-- Right column: properties panel -->
      <aside
        class="min-h-0 w-80 shrink-0 border-l border-border/60 xl:w-88"
      >
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
        class="animate-in zoom-in-95 flex w-full max-w-sm flex-col overflow-hidden rounded-2xl border border-border/60 bg-popover/95 shadow-2xl ring-1 ring-border/40 backdrop-blur-xl duration-150"
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
          {@const isPreparing =
            prepSending !== "done" && !exportHasProgress && !exportFinalizing}
          {@const eta = exportEtaMs()}
          {@const exportDuration = getExportDuration()}
          {@const exportRange = getExportRangeLabel()}
          {@const ringPct = isPreparing
            ? 0
            : exportFinalizing
              ? 100
              : Math.min(100, Math.max(0, displayPct))}
          {@const RING_R = 52}

          <!-- Header: title + live metadata -->
          <header
            class="flex items-center gap-3 border-b border-border/60 px-4 py-3"
          >
            <div class="min-w-0 flex-1">
              <h3
                id="export-dialog-title"
                class="text-[13px] font-semibold tracking-tight text-foreground"
              >
                {#if exportCancelling}
                  Cancelling export…
                {:else if exportFinalizing}
                  Finalising file
                {:else if isPreparing}
                  Preparing export
                {:else}
                  Encoding video
                {/if}
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                {store.exportFormat.toUpperCase()} · {store.exportQuality.toUpperCase()}
                · {formatTime(exportDuration)} clip · {exportRange}
              </p>
            </div>
          </header>

          <!-- Circular progress ring -->
          <div class="flex flex-col items-center gap-3 px-6 pt-5 pb-2">
            <div class="relative size-32" aria-live="polite">
              <svg
                viewBox="0 0 120 120"
                class="size-full -rotate-90 overflow-visible"
              >
                <!-- Track -->
                <circle
                  cx="60"
                  cy="60"
                  r={RING_R}
                  stroke="currentColor"
                  stroke-width="6"
                  class="fill-none text-muted"
                />
                {#if isPreparing}
                  <!-- Indeterminate spinner: a 25-unit arc revolving on a
                       100-unit path. `pathLength="100"` decouples the
                       dash math from `2πr` so floating-point precision
                       can't make the ring sit one pixel short of full. -->
                  <circle
                    cx="60"
                    cy="60"
                    r={RING_R}
                    pathLength="100"
                    stroke="currentColor"
                    stroke-width="6"
                    stroke-linecap="round"
                    class="fill-none text-primary origin-center animate-spin"
                    style="stroke-dasharray: 25 100; animation-duration: 1.2s;"
                  />
                {:else}
                  <!-- Determinate progress with cubic-bezier-eased fill.
                       Dash values live in inline style so they participate
                       in the CSS transition; mixing attribute + style for
                       the same property breaks animation in some engines. -->
                  <circle
                    cx="60"
                    cy="60"
                    r={RING_R}
                    pathLength="100"
                    stroke="currentColor"
                    stroke-width="6"
                    stroke-linecap="round"
                    class="fill-none text-primary"
                    style="stroke-dasharray: 100; stroke-dashoffset: {100 - ringPct}; transition: stroke-dashoffset 220ms cubic-bezier(0.65, 0, 0.35, 1);"
                  />
                  {#if exportFinalizing}
                    <!-- Pulsing tip while we wait on FFmpeg's mux/move. -->
                    <circle
                      cx="60"
                      cy={60 - RING_R}
                      r="3.5"
                      class="fill-primary animate-pulse"
                    />
                  {/if}
                {/if}
              </svg>
              <!-- Centre readout: percentage during encoding, dashes
                   while preparing or finalising. -->
              <div
                class="absolute inset-0 flex flex-col items-center justify-center"
              >
                {#if isPreparing}
                  <span
                    class="text-[11px] uppercase tracking-wider text-muted-foreground"
                    >Prep</span
                  >
                  <span class="text-[10px] text-muted-foreground">…</span>
                {:else if exportFinalizing}
                  <span
                    class="font-mono text-2xl font-semibold tabular-nums text-foreground"
                    >99%</span
                  >
                  <span
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                    >Finalising</span
                  >
                {:else}
                  <span
                    class="font-mono text-2xl font-semibold tabular-nums text-foreground"
                  >
                    {Math.floor(ringPct)}<span
                      class="text-base text-muted-foreground">%</span
                    >
                  </span>
                  {#if eta !== null}
                    <span
                      class="text-[10px] uppercase tracking-wider text-muted-foreground"
                      >~{formatElapsed(eta)} left</span
                    >
                  {:else if exportStartedAt}
                    <span
                      class="text-[10px] uppercase tracking-wider text-muted-foreground"
                      >{formatElapsed(exportNow - exportStartedAt)} elapsed</span
                    >
                  {/if}
                {/if}
              </div>
            </div>

            <!-- Stage list — checkmarks for completed substages, a dot
                 with a subtle pulse for the running one. Collapses to a
                 single "Encoding…" line once Rust takes over. -->
            <ul class="flex flex-col gap-1 self-stretch text-[11px]">
              {#each stages as s}
                {#if !s.skip}
                  <li class="flex items-center gap-2">
                    {#if s.state === "done"}
                      <CheckCircle2 size={11} class="shrink-0 text-success" />
                      <span
                        class="text-muted-foreground line-through decoration-muted-foreground/40"
                        >{s.label}</span
                      >
                    {:else if s.state === "running" && s.key === "ship"}
                      <!-- Beam animation: dots travel through a pipe to suggest
                           the render state being piped to the encoder. -->
                      <span
                        class="ship-beam relative flex h-2.5 w-3.5 shrink-0 items-center overflow-hidden rounded-full bg-primary/15"
                      >
                        <span class="ship-dot ship-dot-1"></span>
                        <span class="ship-dot ship-dot-2"></span>
                        <span class="ship-dot ship-dot-3"></span>
                      </span>
                      <span class="text-foreground">{s.label}</span>
                      <span
                        class="ml-auto font-mono text-[9px] tabular-nums text-muted-foreground"
                        >shipping…</span
                      >
                    {:else if s.state === "running"}
                      <span
                        class="flex size-2.5 shrink-0 items-center justify-center"
                      >
                        <span
                          class="size-1.5 animate-pulse rounded-full bg-primary"
                        ></span>
                      </span>
                      <span class="text-foreground">{s.label}</span>
                    {:else}
                      <span class="size-2.5 shrink-0"></span>
                      <span class="text-muted-foreground/60">{s.label}</span>
                    {/if}
                  </li>
                {/if}
              {/each}
            </ul>
          </div>

          <!-- Footer: kbd hints + cancel -->
          <footer
            class="mt-3 flex h-10 items-center justify-between gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <span class="flex items-center gap-1">
              <Kbd>Esc</Kbd>
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
          <header
            class="flex items-center gap-3 border-b border-border/60 px-4 py-3"
          >
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-success/20 bg-success/10 text-success"
            >
              <CheckCircle2 size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3
                id="export-dialog-title"
                class="text-[13px] font-semibold tracking-tight text-foreground"
              >
                Export complete
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                {store.exportFormat.toUpperCase()} · {store.exportQuality.toUpperCase()}
              </p>
            </div>
          </header>
          <div class="px-4 py-3">
            <p
              class="truncate font-mono text-[10px] text-muted-foreground"
              title={exportResult.path}
            >
              {exportResult.path}
            </p>
          </div>
          <footer
            class="flex h-10 items-center justify-between gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <span class="flex items-center gap-1">
              <Kbd>Esc</Kbd>
              <span>Dismiss</span>
            </span>
            <div class="flex items-center gap-1.5">
              <Button variant="ghost" size="xs" onclick={dismissExportResult}
                >Dismiss</Button
              >
              <Button
                variant="default"
                size="xs"
                class="gap-1.5"
                onclick={revealExportInFolder}
              >
                <FolderOpen size={11} />
                Show in Folder
              </Button>
            </div>
          </footer>
        {:else if exportResult?.kind === "cancelled"}
          <header
            class="flex items-center gap-3 border-b border-border/60 px-4 py-3"
          >
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-muted text-muted-foreground"
            >
              <X size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3
                id="export-dialog-title"
                class="text-[13px] font-semibold tracking-tight text-foreground"
              >
                Export cancelled
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                No file was written.
              </p>
            </div>
          </header>
          <footer
            class="flex h-10 items-center justify-end gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <Button variant="ghost" size="xs" onclick={dismissExportResult}
              >Dismiss</Button
            >
            <Button variant="default" size="xs" onclick={handleExport}
              >Export again</Button
            >
          </footer>
        {:else if exportResult?.kind === "error"}
          <header
            class="flex items-center gap-3 border-b border-border/60 px-4 py-3"
          >
            <div
              class="flex size-8 shrink-0 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive"
            >
              <X size={16} />
            </div>
            <div class="min-w-0 flex-1">
              <h3
                id="export-dialog-title"
                class="text-[13px] font-semibold tracking-tight text-foreground"
              >
                Export failed
              </h3>
              <p class="truncate text-[11px] text-muted-foreground">
                Something went wrong.
              </p>
            </div>
          </header>
          <div
            class="max-h-40 overflow-y-auto border-b border-border/60 px-4 py-3"
          >
            <pre
              class="whitespace-pre-wrap wrap-break-word font-mono text-[10px] text-destructive">{exportResult.message}</pre>
          </div>
          <footer
            class="flex h-10 items-center justify-end gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
          >
            <Button variant="ghost" size="xs" onclick={dismissExportResult}
              >Dismiss</Button
            >
            <Button variant="default" size="xs" onclick={handleExport}
              >Try again</Button
            >
          </footer>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* Hand-off-to-encoder beam: three dots travel left → right with offset
     so it reads as data being piped through. Wrapping container clips,
     dots fade in/out at the track edges. */
  .ship-beam {
    box-shadow: inset 0 0 0 1px hsl(var(--border) / 0.3);
  }
  .ship-dot {
    position: absolute;
    width: 3px;
    height: 3px;
    border-radius: 9999px;
    background: hsl(var(--primary));
    top: 50%;
    transform: translate(-50%, -50%);
    animation: ship-beam-travel 1.1s linear infinite;
  }
  .ship-dot-1 {
    animation-delay: 0s;
  }
  .ship-dot-2 {
    animation-delay: 0.36s;
  }
  .ship-dot-3 {
    animation-delay: 0.72s;
  }
  @keyframes ship-beam-travel {
    0% {
      left: 0%;
      opacity: 0;
    }
    20% {
      opacity: 1;
    }
    80% {
      opacity: 1;
    }
    100% {
      left: 100%;
      opacity: 0;
    }
  }
</style>
