<script lang="ts">
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import ConfirmDialog from "$components/recast/ConfirmDialog.svelte";
  import EditorToolbar from "$components/editor/EditorToolbar.svelte";
  import ExportDialog from "$components/editor/ExportDialog.svelte";
  import ExportFlowDialog, {
    type ExportFlowPhase,
  } from "$components/editor/ExportFlowDialog.svelte";
  import PropertiesPanel from "$components/editor/properity-panel/PropertiesPanel.svelte";
  import Timeline from "$components/editor/Timeline.svelte";
  import VideoPlayerControls from "$components/editor/VideoPlayerControls.svelte";
  import VideoPreview from "$components/editor/VideoPreview.svelte";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import EditorSkeleton from "$components/skeletons/EditorSkeleton.svelte";
  import type { ExportStateEvent } from "$lib/ipc";
  import {
    autosaveProject,
    cancelExport,
    clearAutosave,
    createExportId,
    detectSilence,
    extractWaveform,
    generateThumbnails,
    loadEditorDocument,
    migrateProject,
    openFileLocation,
    refreshTray,
    saveProjectEdits,
  } from "$lib/ipc";
  import { generateAutoZoom } from "$lib/services/analysis";
  import {
    buildCaptionExport,
    buildCloudCaptionTranscript,
    buildExportRenderState,
    runExport,
  } from "$lib/services/export";
  import { isShareSupported, shareRecording } from "$lib/share";
  import { registerShortcutHandlers } from "$lib/shortcuts/registry.svelte";
  import { cloudShare } from "$lib/stores/cloudShare.svelte";
  import {
    createEditorStore,
    type VideoMetadata,
  } from "$lib/stores/editor-store.svelte";
  import { experimentalStore } from "$lib/stores/experimental.svelte";
  import { AudioTimelineEngine } from "$lib/playback/audio-engine";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import {
    createTileProvider,
    type TileProvider,
  } from "$lib/timeline/filmstrip-source";
  import { gdrive } from "$lib/stores/gdrive.svelte";
  import {
    ArrowLeft,
    CheckCircle2,
    Circle,
    Cloud,
    ExternalLink,
    FlaskConical,
    FolderOpen,
    HardDriveUpload,
    Link2,
    LoaderCircle,
    RefreshCw,
    Share2,
    TriangleAlert,
    Upload,
    VolumeX,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { toast } from "@recast/ui/sonner";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onDestroy, onMount, tick, untrack } from "svelte";

  import { log } from "$lib/logger";
  import { cubicOut } from "svelte/easing";
  import { fade, slide } from "svelte/transition";

  interface Props {
    data: {
      filePath: string;
      filename: string;
    };
  }

  let { data }: Props = $props();

  const store = createEditorStore();

  let videoEl: HTMLVideoElement | null = $state(null);
  // True while the WebCodecs engine drives the picture (its clock owns
  // `store.currentTime`). When set, handleTimeUpdate must NOT echo
  // `videoEl.currentTime` — the element free-runs through the un-cut recording,
  // so feeding its time to the store snaps playback back across a cut.
  let webcodecsActive = $state(false);
  // WYSIWYG screenshot (composite, not raw frame); bound from VideoPreview.
  let captureFrame = $state<(() => Promise<Blob | null>) | undefined>(undefined);
  // Loop-within-trim. Lives here because both `ended` and `timeupdate` end-of-clip
  // paths need handling here, with one source of truth for pause-vs-loop.
  let loopEnabled = $state(false);

  // Persisted sidebar/timeline visibility; missing or malformed falls back to all visible.
  const LAYOUT_KEY = "recast-editor-layout";
  function loadLayout(): { sidebar: boolean; timeline: boolean } {
    const fallback = { sidebar: true, timeline: true };
    if (!browser) return fallback;
    try {
      const raw = localStorage.getItem(LAYOUT_KEY);
      if (!raw) return fallback;
      const parsed = JSON.parse(raw) as Partial<typeof fallback>;
      return {
        sidebar:
          typeof parsed?.sidebar === "boolean" ? parsed.sidebar : true,
        timeline:
          typeof parsed?.timeline === "boolean" ? parsed.timeline : true,
      };
    } catch {
      return fallback;
    }
  }
  const initialLayout = loadLayout();
  let showSidebar = $state(initialLayout.sidebar);
  let showTimeline = $state(initialLayout.timeline);

  $effect(() => {
    if (!browser) return;
    try {
      localStorage.setItem(
        LAYOUT_KEY,
        JSON.stringify({ sidebar: showSidebar, timeline: showTimeline }),
      );
    } catch {
      // localStorage can throw in private-mode/quota edge cases — the toggle
      // still works for the session, it just won't be remembered.
    }
  });

  let previewContainerEl: HTMLDivElement | null = $state(null);
  let systemAudioEl: HTMLAudioElement | null = $state(null);
  let micAudioEl: HTMLAudioElement | null = $state(null);
  let videoSrc = $state("");
  let systemAudioSrc = $state("");
  let micAudioSrc = $state("");
  // Sample-accurate cut-aware audio for the WebCodecs preview (no seeking → no
  // drift). Falls back to the <audio> elements if it can't init/decode.
  let audioEngine: AudioTimelineEngine | null = $state(null);
  let audioEngineTried = false;
  let audioEngineFailed = $state(false);
  let cursorPath = $state<string | null>(null);
  let cameraPath = $state<string | null>(null);
  let cameraSrc = $state("");
  let documentPath = $state("");
  let isLoading = $state(true);
  let error = $state("");
  let loadedPath = $state("");
  let thumbnailToken = 0;

  // Density-based filmstrip: a WebCodecs tile provider (or null, when the clip
  // bar falls back to the stretched Rust strip). `filmstripVersion` bumps as
  // decoded tiles land so the bar repaints. Clip-bar height (h-12) in CSS px.
  const FILMSTRIP_TILE_HEIGHT = 48;
  let tileProvider = $state<TileProvider | null>(null);
  let filmstripVersion = $state(0);
  let tileProviderToken = 0;

  // Legacy-format gate: a v1 `.recast` must be migrated before the editor
  // touches it. `migrationDone` distinguishes a confirmed update (→ reload)
  // from a dismissal (→ leave, don't open an un-migrated project).
  let showMigration = $state(false);
  let migrationDone = false;

  // Autosave: save edit state every 30 seconds while editing.
  const AUTOSAVE_INTERVAL_MS = 30_000;
  let autosaveTimer: ReturnType<typeof setInterval> | null = null;

  function startAutosave() {
    stopAutosave();
    autosaveTimer = setInterval(async () => {
      if (!documentPath || isLoading) return;
      // Skip the full-state serialize when nothing changed since the last
      // save/autosave — most idle ticks are clean, so the 30s timer stays off
      // the main thread entirely until there's real work to persist.
      if (!store.isDirty) return;
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
    log.clearRecast();
    // Clear autosave on clean exit.
    if (documentPath) {
      clearAutosave(documentPath).catch(() => {});
    }
  });

  // Seek video + audio back to trimStart and resume. Used by both loop paths
  // (timeupdate and ended); returns true so the timeupdate handler can bail.
  function loopBackToStart(): boolean {
    if (!videoEl) return false;
    const start = store.trimStart || 0;
    videoEl.currentTime = start;
    for (const el of [systemAudioEl, micAudioEl]) {
      if (el) el.currentTime = start;
    }
    // play() can reject (user-gesture) — log instead of stalling silently.
    void videoEl.play().catch((err) => {
      console.warn("loop replay failed:", err);
    });
    store.isPlaying = true;
    return true;
  }

  function handleTimeUpdate() {
    if (!videoEl) return;
    if (store.isPlaying) {
      // Legacy <video> path only: in the WebCodecs path the clock owns time and
      // audio, so echoing this element's time would fight it across cuts.
      if (webcodecsActive) return;
      store.currentTime = videoEl.currentTime;
      // Loop only matters when trimEnd is below the natural duration; the
      // natural end uses the `ended` event (more precise than the ~250ms tick).
      if (loopEnabled && store.metadata) {
        const trimEnd = store.trimEnd > 0 ? store.trimEnd : store.metadata.duration;
        if (trimEnd > 0 && trimEnd < store.metadata.duration - 0.05) {
          if (videoEl.currentTime >= trimEnd - 0.05) {
            loopBackToStart();
            return;
          }
        }
      }
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
    // Loop wins over stop-at-end. The short-circuit avoids the pause calls below
    // racing loopBackToStart and the audio effect batching out the false→true flip.
    if (loopEnabled && videoEl) {
      loopBackToStart();
      return;
    }
    store.isPlaying = false;
    systemAudioEl?.pause();
    micAudioEl?.pause();
  }

  // Slave the audio (full-recording WAVs) to the cut-aware picture clock so they
  // skip the same cuts. Normal playback stays locked at 1×; the only corrections
  // are one snap per cut boundary and per seek. Steady-state drift past this
  // threshold hard-seeks audio back; loose enough the ~25Hz publish doesn't thrash.
  const AUDIO_SYNC_THRESHOLD = 0.12;
  // A cut crossing or scrub jumps the playhead far past one publish quantum;
  // detecting it snaps audio exactly on cuts of any length, including short ones.
  const AUDIO_JUMP = 0.12;
  let audioSyncRaf: number | null = null;
  let lastAudioTarget = -1;
  function syncAudioToClock() {
    audioSyncRaf = requestAnimationFrame(syncAudioToClock);
    if (!store.isPlaying) {
      lastAudioTarget = -1;
      return;
    }
    // WebCodecs path: the gapless output clock owns time. Legacy <video> path:
    // the <video> element is the master and ALREADY skips cuts (it jumps to
    // cut.end at each boundary), so tracking it here makes the <audio> elements
    // skip the same cuts within a frame. Without this, the 4 Hz `timeupdate`
    // drift-check left audio playing the removed region for up to ~250 ms.
    const target = webcodecsActive
      ? store.currentTime
      : (videoEl?.currentTime ?? store.currentTime);
    const jumped =
      lastAudioTarget < 0 || Math.abs(target - lastAudioTarget) > AUDIO_JUMP;
    lastAudioTarget = target;
    for (const el of [systemAudioEl, micAudioEl]) {
      // CRITICAL: never stack a seek on an element that's still seeking (e.g.
      // cold-start buffering) — each new currentTime= interrupts the last, so it
      // never settles and the audio cuts out entirely. Wait for the current seek.
      if (!el || el.paused || el.seeking || el.readyState < 2) continue;
      // Snap exactly on a cut/seek (any length), or when steady drift grows.
      if (jumped || Math.abs(el.currentTime - target) > AUDIO_SYNC_THRESHOLD) {
        el.currentTime = target;
      }
    }
  }
  function startAudioClockSync() {
    if (audioSyncRaf === null) audioSyncRaf = requestAnimationFrame(syncAudioToClock);
  }
  function stopAudioClockSync() {
    if (audioSyncRaf !== null) {
      cancelAnimationFrame(audioSyncRaf);
      audioSyncRaf = null;
    }
  }
  onDestroy(stopAudioClockSync);
  onDestroy(() => audioEngine?.dispose());
  onDestroy(disposeTileProvider);

  // Kept audio regions and current OUTPUT time — what the Web Audio engine
  // schedules against. Regions are the kept SEGMENTS (trim − cuts, split-bounded)
  // each carrying its clip speed, so audio speeds up/down with the segment.
  // Output time is the warped axis (store.timeMap), matching the picture clock.
  function audioRegions() {
    return store.segments.map((s) => ({
      start: s.start,
      end: s.end,
      speed: store.segmentSpeedAt(s.start),
    }));
  }
  function outputNow() {
    return originalToOutput(store.timeMap, store.currentTime);
  }
  // Lazily build the engine on first WebCodecs playback. Tried once; on failure
  // it's marked failed and the <audio> elements take over.
  async function ensureAudioEngine() {
    if (audioEngine || audioEngineTried) return;
    audioEngineTried = true;
    if (!systemAudioSrc && !micAudioSrc) {
      audioEngineFailed = true;
      return;
    }
    try {
      const eng = await AudioTimelineEngine.create([
        systemAudioSrc || null,
        micAudioSrc || null,
      ]);
      const s = store.audioSettings;
      eng.setVolume(s.volume, s.muted);
      audioEngine = eng;
    } catch (err) {
      console.warn("Web Audio engine unavailable; using <audio> fallback:", err);
      audioEngineFailed = true;
    }
  }

  // Play/pause audio in lockstep with `isPlaying`. WebCodecs path drives the
  // Web Audio engine; the <audio> elements are the fallback / legacy path.
  $effect(() => {
    const playing = store.isPlaying;
    const wc = webcodecsActive;
    const eng = audioEngine;
    const failed = audioEngineFailed;

    if (wc && !failed) {
      // Engine owns audio here: keep the <audio> elements and the seek loop off.
      for (const el of [systemAudioEl, micAudioEl]) el?.pause();
      stopAudioClockSync();
      if (playing) {
        void ensureAudioEngine();
        if (eng) {
          void eng.play(
            untrack(() => audioRegions()),
            untrack(() => outputNow()),
          );
        }
      } else {
        eng?.pause();
      }
      return;
    }

    // Fallback (engine failed) or legacy <video> path: slave the <audio>
    // elements to the playhead, and make sure the engine is silent.
    audioEngine?.pause();
    const alignTo = untrack(() =>
      wc ? store.currentTime : (videoEl?.currentTime ?? 0),
    );
    for (const el of [systemAudioEl, micAudioEl]) {
      if (!el) continue;
      if (playing) {
        el.currentTime = alignTo;
        void el.play().catch((err) => {
          console.warn("Audio play failed:", err);
        });
      } else {
        el.pause();
      }
    }
    // Run the rAF sync whenever the <audio> elements are the audio source —
    // both the legacy <video> path AND the engine-failed WebCodecs fallback.
    // It keeps them locked to the master (video time / output clock) so cuts
    // are skipped tightly, not just on the coarse `timeupdate` tick.
    if (playing) startAudioClockSync();
    else stopAudioClockSync();
  });

  // Reschedule the engine only on a seek/loop (output jump) or a kept-regions
  // edit. Crossing a cut doesn't move gapless OUTPUT time, so it doesn't trigger.
  const ENGINE_RESEEK_JUMP = 0.15;
  let engineSyncOut = -1;
  let lastRegionsKey = "";
  $effect(() => {
    const t = store.currentTime;
    const eng = audioEngine;
    if (!eng || !webcodecsActive || !store.isPlaying) {
      engineSyncOut = -1;
      lastRegionsKey = "";
      return;
    }
    const out = originalToOutput(store.timeMap, t);
    const regions = audioRegions();
    const regionsKey = regions
      .map((r) => `${r.start.toFixed(3)}-${r.end.toFixed(3)}@${r.speed.toFixed(3)}`)
      .join(",");
    const jumped =
      engineSyncOut >= 0 && Math.abs(out - engineSyncOut) > ENGINE_RESEEK_JUMP;
    const editsChanged = lastRegionsKey !== "" && regionsKey !== lastRegionsKey;
    engineSyncOut = out;
    lastRegionsKey = regionsKey;
    if (jumped || editsChanged) eng.reschedule(regions, out);
  });

  // Legacy/fallback path: the <audio> elements are slaved to the <video> clock,
  // so they must share its per-segment clip speed or audio plays at 1× while the
  // picture speeds up. preservesPitch stays on (default), matching the export's
  // pitch-preserving atempo. On the WebCodecs path these elements are paused
  // (the Web Audio engine carries speed via the schedule), so this is a no-op.
  $effect(() => {
    const segSpeed = store.segmentSpeedAtTime(store.currentTime);
    if (systemAudioEl) systemAudioEl.playbackRate = segSpeed;
    if (micAudioEl) micAudioEl.playbackRate = segSpeed;
  });

  // Apply volume/mute from the store's audio settings to both audio elements.
  $effect(() => {
    const settings = store.audioSettings;
    const vol = settings.muted
      ? 0
      : Math.max(0, Math.min(1, settings.volume / 100));
    if (systemAudioEl) systemAudioEl.volume = vol;
    if (micAudioEl) micAudioEl.volume = vol;
    audioEngine?.setVolume(settings.volume, settings.muted);
  });

  // Transport seek for `store.seek()` — seeks from outside the player (a
  // transcript line, chapters, …). Most in-player seeks (timeline scrub,
  // frame-step) already set `videoEl.currentTime` themselves; this gives panels
  // the same reach. Moving the <video> works for both the legacy path and the
  // WebCodecs path: paused → `seeked` realigns the picture clock; playing → the
  // draw loop re-seats the clock off the changed `store.currentTime`. Setting
  // `currentTime` alone failed mid-playback because the next time-publish (legacy)
  // overwrote it before the seek took.
  $effect(() => {
    const off = store.registerSeekHandler((t) => {
      if (videoEl) videoEl.currentTime = t;
      for (const el of [systemAudioEl, micAudioEl]) {
        if (el) el.currentTime = t;
      }
    });
    return off;
  });

  // Snap audio to the video time on scrub. Skipped on the WebCodecs path, where
  // audio follows the clock and snapping to seeks would fight it.
  function handleVideoSeeked() {
    if (!videoEl || webcodecsActive) return;
    const t = videoEl.currentTime;
    // Publish the jumped position immediately. During playback the <video>
    // cut-skip seeks to cut.end, but `store.currentTime` otherwise only catches
    // up on the next 4 Hz `timeupdate` — so captions/overlays (which key off
    // `store.currentTime`) lagged the cut by up to ~250 ms. Snap them here.
    store.currentTime = t;
    for (const el of [systemAudioEl, micAudioEl]) {
      if (el) el.currentTime = t;
    }
  }

  // Frame-step on the OUTPUT axis so stepping across a cut lands on the next
  // kept frame, never inside a removed range. `store.currentTime` stays original.
  function frameStepSeek(direction: 1 | -1) {
    if (!store.metadata) return;
    const map = store.timeMap;
    const frameDur = 1 / (store.metadata.fps || 30);
    const outDur = originalToOutput(map, store.metadata.duration);
    const nextOut = Math.max(
      0,
      Math.min(originalToOutput(map, store.currentTime) + frameDur * direction, outDur),
    );
    const orig = outputToOriginal(map, nextOut);
    if (videoEl) videoEl.currentTime = orig;
    store.currentTime = orig;
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

  function disposeTileProvider() {
    tileProvider?.dispose();
    tileProvider = null;
  }

  // Build the WebCodecs filmstrip provider for the opened media. Tokened so a
  // rapid reopen disposes a provider that resolves after we moved on.
  async function setupTileProvider(url: string) {
    const token = ++tileProviderToken;
    disposeTileProvider();
    const dpr = browser ? window.devicePixelRatio || 1 : 1;
    const provider = await createTileProvider({
      url,
      sizeBytes: store.metadata?.sizeBytes,
      tileHeightPx: Math.round(FILMSTRIP_TILE_HEIGHT * dpr),
      onChange: () => {
        filmstripVersion++;
      },
    });
    if (token !== tileProviderToken) {
      provider?.dispose();
      return;
    }
    tileProvider = provider;
  }

  async function loadThumbnailStrip(path: string) {
    // Skip without a usable duration: bumping the token would cancel an in-flight
    // strip, and a 0-duration source just yields black frames.
    const duration = store.metadata?.duration ?? 0;
    if (duration <= 0) return;

    const token = ++thumbnailToken;
    try {
      const count = duration > 60 ? 12 : 8;
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

  // Latch so the lazy scheduler fires once per loaded clip (reset on load).
  let waveformRequested = false;

  // Decode the audio peak envelope for the timeline waveform. Best-effort async.
  async function loadWaveform() {
    // Skip sub-5s clips: too narrow to read, and the FFmpeg pass isn't worth it.
    const duration = store.metadata?.duration ?? 0;
    if (duration > 0 && duration < 5) {
      store.waveform = [];
      return;
    }
    try {
      store.waveform = await extractWaveform(
        store.audioPath,
        store.microphonePath,
      );
    } catch (err) {
      console.warn("Waveform extraction failed", err);
      store.waveform = [];
    }
    // Warm the silence-detection cache off the back of the waveform pass (one
    // FFmpeg decode at a time, never on the load path). The result is discarded
    // here — `detectSilence` writes it to the file-identity cache the review
    // popover reads, so opening that popover is instant. Default options match
    // the popover's "balanced" sensitivity.
    void warmSilenceCache();
  }

  async function warmSilenceCache() {
    try {
      await detectSilence(
        store.audioPath,
        store.microphonePath,
        store.cursorPath,
      );
    } catch (err) {
      console.warn("Silence precompute failed", err);
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
    cameraPath = null;
    cameraSrc = "";
    videoEl?.pause();
    systemAudioEl?.pause();
    micAudioEl?.pause();
    // Tear down the previous file's engine; it rebuilds on first play.
    audioEngine?.dispose();
    audioEngine = null;
    audioEngineTried = false;
    audioEngineFailed = false;
    store.metadata = null;
    store.reset();
    store.thumbnailStrip = [];
    disposeTileProvider();

    try {
      const document = await loadEditorDocument(data.filePath);
      if (document.needsMigration) {
        // Stop before loading anything — prompt to update the format first.
        isLoading = false;
        showMigration = true;
        return;
      }
      documentPath = document.projectPath;
      store.videoPath = document.projectPath;
      store.metadata = document.metadata;
      store.loadRenderState(document.renderState);
      // Scope every subsequent log in this window to the opened recast.
      log.setRecast(documentPath, {
        width: document.metadata.width,
        height: document.metadata.height,
        durationSec: Math.round(document.metadata.duration),
        fps: document.metadata.fps,
        codec: document.metadata.codec,
      });
      void loadThumbnailStrip(document.projectPath);
      videoSrc = convertFileSrc(document.mediaPath);
      void setupTileProvider(videoSrc);
      cursorPath = document.cursorPath ?? null;
      store.cursorPath = cursorPath;
      // Raw on-disk media paths for Rust-side analysis (silence detection).
      store.recordingPath = document.mediaPath;
      store.audioPath = document.audioPath ?? null;
      store.microphonePath = document.microphonePath ?? null;
      store.waveform = [];
      // Lazy: the idle-scheduled effect below extracts the waveform once the
      // editor is interactive, so the ffmpeg pass never competes with load.
      waveformRequested = false;
      systemAudioSrc = document.audioPath
        ? convertFileSrc(document.audioPath)
        : "";
      micAudioSrc = document.microphonePath
        ? convertFileSrc(document.microphonePath)
        : "";
      cameraPath = document.cameraPath ?? null;
      cameraSrc = cameraPath ? convertFileSrc(cameraPath) : "";
      // Mount the editor body (VideoPreview renders only when !isLoading) so the
      // <video> exists before load().
      isLoading = false;
      await tick();
      videoEl?.load();
      systemAudioEl?.load();
      micAudioEl?.load();
      void maybeRunAutoZoom();
    } catch (err) {
      console.error("Failed to load editor document", err);
      log.error("session", "recast_load_failed", { error: String(err) });
      error = `Could not load project: ${err}`;
      isLoading = false;
    }
  }

  // Throwing here keeps ConfirmDialog open with the error shown.
  async function confirmMigration() {
    await migrateProject(data.filePath);
    migrationDone = true;
  }

  function onMigrationOpenChange(open: boolean) {
    if (open) return;
    if (migrationDone) {
      migrationDone = false;
      void loadDocument();
    } else {
      void goto("/recasts");
    }
  }

  // On first load, place a focus region at each detected click + settle. The
  // `autoZoomApplied` document flag stops reopens from repopulating cleared regions.
  let autoZoomRunning = false;

  async function maybeRunAutoZoom() {
    if (autoZoomRunning) return;
    if (!store.autoZoomEnabled || store.autoZoomApplied) return;
    if (!cursorPath) {
      // No cursor track to analyse — latch the flag so we don't retry on reopen.
      store.autoZoomApplied = true;
      return;
    }
    if (store.zoomRegions.length > 0) {
      // Regions already exist (autosave-restored or manual) — skip silently.
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
      // generateAutoZoom latches store.autoZoomApplied itself on non-error paths.
      const outcome = await generateAutoZoom(store, cursorPath, {
        documentPath,
      });
      if (outcome.reason === "bad-bounds") return;
      if (outcome.applied > 0) {
        toast.success(
          `Added ${outcome.applied} focus moment${outcome.applied === 1 ? "" : "s"}`,
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

  // Re-run is exposed to FocusPanel via a window CustomEvent so the nested panel
  // doesn't thread a prop through every component.
  $effect(() => {
    function onRerun() {
      store.clearAutoZooms();
      store.autoZoomApplied = false;
      void runAutoZoom({ silentEmpty: false });
    }
    window.addEventListener("recast:rerun-auto-zoom", onRerun);
    return () => window.removeEventListener("recast:rerun-auto-zoom", onRerun);
  });

  // Export lifecycle UI state — in the route, not the store, since the overlay
  // owns success/cancel/error reveals that don't belong in global state.
  let exportStartedAt = $state<number>(0);
  let exportNow = $state<number>(Date.now());
  let exportCancelling = $state(false);
  let exportFinalizing = $state(false);
  let exportHasProgress = $state(false);
  let activeExportId = $state<string | null>(null);
  // Backend prep sub-step (e.g. "Rendering cursor & annotations") emitted by Rust
  // while it runs the synchronous prep passes AFTER hand-off but before the encode
  // reports real progress — otherwise the checklist's "Encode frames" step sits
  // stalled during that window.
  let exportPrepDetail = $state<string | null>(null);


  // Rotating status messages shown below the progress ring during encode.
  const ENCODE_MESSAGES = [
    "Crunching frames",
    "Encoding pixels",
    "Weaving the timeline",
    "Tuning the colours",
    "Squeezing the bitrate",
    "Polishing every frame",
  ];
  let encodeMessageIndex = $state(0);

  // Preparing-stage substages, surfaced in the dialog instead of a generic spinner.
  let prepText = $state<"pending" | "running" | "done">("pending");
  let prepCursor = $state<"pending" | "running" | "done">("pending");
  let prepSending = $state<"pending" | "running" | "done">("pending");
  function resetPrep() {
    prepText = "pending";
    prepCursor = "pending";
    prepSending = "pending";
  }

  // Eased display percentage: raw FFmpeg progress is jumpy, so lerp the ring
  // toward it each animation tick while exporting.
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
      // Critically-damped follower (~250ms tau): ease-out toward target, no overshoot.
      const tau = 250;
      const k = 1 - Math.exp(-dt / tau);
      const next = displayPct + (target - displayPct) * k;
      // Never animate backwards; the export is monotonic so the ring should be too.
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

  // ETA from elapsed × (1 − pct) / pct; only meaningful past ≥10% progress.
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
      // Refresh the tray's Recent Exports. `null` = "list changed, leave the
      // recording flag alone" (the panel window owns that).
      void refreshTray(null).catch(() => {});
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
      case "preparing":
        exportPrepDetail = event.detail ?? null;
        return;
      case "progress": {
        const next = Math.min(Math.max(event.progress, 0), 100);
        const current = store.exportProgress ?? 0;

        // FFmpeg progress is noisy near the end on some Windows builds; keep the
        // bar monotonic and ignore sub-0.1% jitter so it doesn't flicker at 99%.
        if (!exportHasProgress || next >= 100 || next > current + 0.1) {
          store.exportProgress = Math.max(current, next);
        }
        exportHasProgress = true;
        // Safety net only: Rust emits a real `finalizing` event now, so this just
        // catches the rare case where that event is dropped.
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
    exportPrepDetail = null;
    activeExportId = exportId;
    exportResult = null;
    exportStartedAt = Date.now();
    exportNow = exportStartedAt;
    resetPrep();

    try {
      // Build the payload Rust renders (text→PNG, cursor→sprite sheet); the
      // hooks drive the "Preparing…" sub-stages.
      const { renderState: finalRenderState, metadata: meta } =
        await buildExportRenderState(store, {
          hooks: {
            onText: (s) => (prepText = s),
            onCursor: (s) => (prepCursor = s),
            onSending: (s) => (prepSending = s),
          },
        });

      // The settings this export ran with — key when a user reports a bad export.
      log.info("export", "export_started", {
        exportId,
        format: store.exportFormat,
        quality: store.exportQuality,
        speed: store.exportSpeed,
        gif: store.exportFormat === "gif" ? store.gifSettings : undefined,
        annotations: finalRenderState.annotations.length,
        zoomRegions: finalRenderState.zoomRegions.length,
        cuts: finalRenderState.cuts?.length ?? 0,
        padding: finalRenderState.padding ?? 0,
        durationSec: meta ? Math.round(meta.duration) : undefined,
      });
      const path = await runExport({
        inputPath: documentPath || data.filePath,
        format: store.exportFormat,
        quality: store.exportQuality,
        renderState: finalRenderState,
        exportId,
        gifSettings:
          store.exportFormat === "gif" ? store.gifSettings : undefined,
        speed: store.exportSpeed,
        // GIF carries fps in gifSettings; MP4/WebM use the picker (null = source).
        fps: store.exportFormat === "gif" ? undefined : store.exportFps,
        // No-op unless a transcript exists and caption options are enabled.
        captions: buildCaptionExport(store),
        // Drop stray events from a run the user already cancelled or replaced,
        // so a background teardown can't disturb the current flow (or re-toast).
        onState: (e) => {
          if (activeExportId === exportId) handleExportState(e);
        },
      });
      // Fall back to the Promise result if the success event was missed.
      if (!exportResult) {
        setExportResult({ kind: "success", path });
      }
      log.info("export", "export_completed", {
        exportId,
        elapsedMs: Date.now() - exportStartedAt,
      });
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
          log.info("export", "export_cancelled", { exportId });
        } else {
          console.error("Export failed:", err);
          log.error("export", "export_failed", { exportId, message });
          setExportResult({ kind: "error", message });
        }
      }
    } finally {
      // Only tear down shared export state if THIS run still owns the flow.
      // After an optimistic cancel the user may have already kicked off a new
      // export; this (old) run completing in the background must not clobber the
      // new one's isExporting/progress.
      if (activeExportId === exportId) {
        activeExportId = null;
        store.isExporting = false;
        store.exportProgress = null;
        exportHasProgress = false;
        exportCancelling = false;
        exportFinalizing = false;
      }
    }
  }

  async function handleCancelExport() {
    if (!store.isExporting || !activeExportId) return;
    const id = activeExportId;
    // Optimistic cancel: signal the backend, then release the UI to "cancelled"
    // immediately instead of waiting for ffmpeg to die. The pipeline still kills
    // ffmpeg (watchdog, ≤250ms) and removes the partial file in the background;
    // its eventual rejection is a no-op (exportResult is already set, and the
    // finally only clears a still-matching activeExportId). This keeps cancel
    // snappy so the user can get back to editing right away.
    void cancelExport(id).catch((err) => console.warn("cancel_export failed", err));
    store.isExporting = false;
    setExportResult({ kind: "cancelled" });
  }

  function dismissExportResult() {
    exportResult = null;
  }

  // Options phase is UI-only (the picker before Export); progress/result phases
  // derive from the pipeline state, so the dialog is one surface that morphs.
  let exportOptionsOpen = $state(false);
  const exportPhase: ExportFlowPhase | null = $derived(
    store.isExporting
      ? "progress"
      : exportResult?.kind === "success"
        ? "success"
        : exportResult?.kind === "cancelled"
          ? "cancelled"
          : exportResult?.kind === "error"
            ? "error"
            : exportOptionsOpen
              ? "options"
              : null,
  );
  const isExportFlowOpen = $derived(exportPhase !== null);

  // Silence cuts only. Manual ripple deletes are always honoured, so they must
  // not trip the "enable Silence detection" banner — only auto cuts depend on it.
  const silenceCutCount = $derived(
    store.cuts.filter((c) => c.source === "silence").length,
  );

  function openExportOptions() {
    if (store.isExporting) return;
    exportOptionsOpen = true;
  }

  function dismissExportOptions() {
    exportOptionsOpen = false;
  }

  function confirmExportOptions() {
    exportOptionsOpen = false;
    void handleExport();
  }

  // Esc per phase: cancel a running export, dismiss a finished one, close the
  // picker. Backdrop click is the same but never cancels a running export.
  function handleExportEscape() {
    if (store.isExporting) {
      void handleCancelExport();
      return;
    }
    if (exportResult) {
      dismissExportResult();
      return;
    }
    if (exportOptionsOpen) {
      dismissExportOptions();
    }
  }

  function handleExportBackdrop() {
    if (store.isExporting) return;
    if (exportResult) {
      dismissExportResult();
      return;
    }
    if (exportOptionsOpen) dismissExportOptions();
  }

  async function revealExportInFolder() {
    if (exportResult?.kind !== "success") return;
    try {
      await openFileLocation(exportResult.path);
    } catch (err) {
      toast.error(`Could not open folder: ${err}`);
    }
  }

  // Push the latest export to Drive, or route to Settings to connect first
  // (connecting opens a browser tab, which can't happen from this card).
  async function uploadExportToDrive() {
    if (exportResult?.kind !== "success") return;
    await gdrive.init();
    if (!gdrive.connected) {
      toast.info("Connect Google Drive in Settings first.");
      void goto("/settings");
      return;
    }
    try {
      await gdrive.upload(exportResult.path);
      // Progress surfaces inline via successUpload and the corner-notifications
      // store, so the upload stays trackable after dismissing the card.
    } catch (e) {
      toast.error(`Drive upload failed: ${e}`);
    }
  }

  // Share the export to Recast Cloud and copy the link; routes to Settings if
  // not signed in. Progress surfaces through corner-notifications (phase-based).
  async function shareCurrentExportToCloud() {
    if (exportResult?.kind !== "success") return;
    await cloudShare.init();
    if (!cloudShare.signedIn) {
      toast.info("Sign in to Recast Cloud in Settings first.");
      void goto("/settings");
      return;
    }
    const title =
      exportResult.path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "Recast";
    try {
      const result = await cloudShare.share(
        exportResult.path,
        title,
        undefined,
        buildCloudCaptionTranscript(store),
      );
      try {
        await navigator.clipboard.writeText(result.shareUrl);
        toast.success("Shared. Link copied to clipboard.");
      } catch {
        toast.success("Shared to Recast Cloud.");
      }
    } catch (e) {
      toast.error(`Cloud share failed: ${(e as Error)?.message ?? e}`);
    }
  }

  // The success card's path, so the upload state can key off it. Null unless the
  // dialog is in the success state.
  const successPath = $derived(
    exportResult?.kind === "success" ? exportResult.path : null,
  );
  // Most-recent upload for the exported file; drives the inline progress in the
  // success card and survives status transitions.
  const successUpload = $derived.by(() => {
    if (!successPath) return undefined;
    const list = gdrive.activeUploads.filter(
      (u) => u.sourcePath === successPath,
    );
    list.sort((a, b) => b.uploadId.localeCompare(a.uploadId));
    return list[0];
  });
  const successUploadPct = $derived(
    successUpload && successUpload.totalBytes
      ? Math.min(
          100,
          Math.round(
            (successUpload.bytesSent / successUpload.totalBytes) * 100,
          ),
        )
      : 0,
  );

  async function copyDriveLink(link: string) {
    try {
      await navigator.clipboard.writeText(link);
      toast.success("Drive link copied.");
    } catch (e) {
      toast.error(`Could not copy link: ${e}`);
    }
  }

  // `navigator.share` exposure is static; sample once so the button renders
  // without a reactive read.
  const shareSupported = isShareSupported();

  async function shareExportedFile() {
    if (exportResult?.kind !== "success") return;
    const fileName = exportResult.path.split(/[\\/]/).pop() ?? "recording";
    const fallbackLink =
      successUpload?.status === "complete" ? successUpload.webViewLink : undefined;
    const result = await shareRecording({
      path: exportResult.path,
      fileName,
      title: fileName,
      text: "Made with Recast",
      fallbackLink,
    });
    if (result.ok || result.reason === "cancelled") return;
    if (result.reason === "unsupported") {
      toast.error(
        fallbackLink
          ? "Sharing isn't available on this device."
          : "Sharing files isn't available here. Upload to Drive first to share a link.",
      );
    } else {
      toast.error(`Share failed: ${result.message ?? "unknown error"}`);
    }
  }

  async function openDriveLink(link: string) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(link);
    } catch {
      window.open(link, "_blank", "noopener");
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


  let isSaving = $state(false);

  async function handleSave() {
    if (!documentPath || isSaving || isLoading) return;
    isSaving = true;
    // Paint the saving state before the synchronous serialize so the button
    // reflects the click immediately. The serialize itself stays on the main
    // thread by necessity — Tauri's IPC bridge JSON-encodes command args on the
    // main thread anyway, so a worker would only add a proxy-stripping clone of
    // equal cost; the win is gating autosave on isDirty (see startAutosave).
    await tick();
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

  // Bind the editor's mod-combo shortcuts to the central registry for the life
  // of this route. Each bails while the export flow dialog owns the screen.
  onMount(() =>
    registerShortcutHandlers({
      "editor.undo": () => {
        if (!isExportFlowOpen) store.undo();
      },
      "editor.redo": () => {
        if (!isExportFlowOpen) store.redo();
      },
      "editor.save": () => {
        if (!isExportFlowOpen) void handleSave();
      },
      "editor.toggleSidebar": () => {
        if (!isExportFlowOpen) showSidebar = !showSidebar;
      },
      "editor.toggleTimeline": () => {
        if (!isExportFlowOpen) showTimeline = !showTimeline;
      },
    }),
  );

  function handleKeydown(e: KeyboardEvent) {
    // Bail on auto-repeat so a held key counts once.
    if (e.defaultPrevented || e.repeat) return;

    // The flow dialog owns Esc routing; bail so global shortcuts don't fire under it.
    if (isExportFlowOpen) return;

    // Never hijack typing in inputs / textareas / contenteditable.
    const target = e.target;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    ) {
      return;
    }

    // Mod-combo shortcuts are owned by the central registry; bail on Ctrl/⌘ so a
    // combo never trips a plain-key action below.
    if (e.ctrlKey || e.metaKey) return;

    // Plain keys: play/pause, frame step, fullscreen.
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
        if (store.metadata) frameStepSeek(-1);
        break;
      case "ArrowRight":
        if (store.metadata) frameStepSeek(1);
        break;
      case "f":
      case "F":
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

  // Extract the waveform lazily: defer to browser idle (best-effort) so the
  // ffmpeg pass runs after the editor is interactive, never on the load path.
  // The latch keeps the reactive re-runs from scheduling it more than once.
  $effect(() => {
    if (store.waveform.length > 0 || waveformRequested) return;
    if (!store.audioPath && !store.microphonePath) return;
    waveformRequested = true;
    const run = () => void loadWaveform();
    if (typeof requestIdleCallback === "function") {
      requestIdleCallback(run, { timeout: 3000 });
    } else {
      setTimeout(run, 1000);
    }
  });

  $effect(() => {
    if (!store.isExporting) return;
    exportNow = Date.now();
    // Elapsed-time timer for the status strip.
    const timer = setInterval(() => {
      exportNow = Date.now();
    }, 500);
    return () => clearInterval(timer);
  });

  // Cycle the encode status messages while an export is running.
  $effect(() => {
    if (!store.isExporting) {
      encodeMessageIndex = 0;
      return;
    }
    const timer = setInterval(() => {
      encodeMessageIndex = (encodeMessageIndex + 1) % ENCODE_MESSAGES.length;
    }, 2600);
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
      // Live hint: show the backend's prep sub-step ("Rendering cursor &
      // annotations") until real encode progress arrives, so this row is never a
      // stalled "Encode frames · pending" during the Rust prep window.
      label: exportFinalizing
        ? "Finalise file"
        : !exportHasProgress && exportPrepDetail
          ? exportPrepDetail
          : "Encode frames",
      state:
        prepSending !== "done"
          ? "pending"
          : exportFinalizing || exportHasProgress || exportPrepDetail !== null
            ? "running"
            : "pending",
    },
  ]);
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="fixed inset-0 flex min-h-screen w-full flex-col overflow-hidden bg-background text-foreground"
>
  <CustomTitlebar wrapperClass="h-9">
    <EditorToolbar
      {store}
      filename={data.filename}
      onexport={openExportOptions}
      onsave={handleSave}
      {isSaving}
      {showSidebar}
      {showTimeline}
      onToggleSidebar={() => (showSidebar = !showSidebar)}
      onToggleTimeline={() => (showTimeline = !showTimeline)}
    />
  </CustomTitlebar>

  <ConfirmDialog
    bind:open={showMigration}
    title="Update project format"
    description="This project was made with an older version of Recast. Update it to the current format to keep editing. A backup (.bak) is saved next to it first."
    confirmLabel="Update project"
    cancelLabel="Not now"
    onConfirm={confirmMigration}
    onOpenChange={onMigrationOpenChange}
  />

  <!-- Project has silence cuts but the flag is off, so they're hidden and
       skipped on export — surface an inline opt-in so work isn't lost. -->
  {#if !isLoading && !error && silenceCutCount > 0 && !experimentalStore.silenceDetection}
    <div
      class="flex items-center gap-2.5 border-b border-warning/30 bg-warning/10 px-3 py-1.5 text-[11px] text-warning"
      role="status"
    >
      <FlaskConical class="size-3.5 shrink-0" />
      <VolumeX class="size-3.5 shrink-0" />
      <span class="min-w-0 flex-1 truncate">
        This project has {silenceCutCount} silence cut{silenceCutCount === 1
          ? ""
          : "s"}, currently hidden and skipped on export. Enable
        <span class="font-semibold">Silence detection</span> to use them.
      </span>
      <Button
        variant="outline"
        size="xs"
        class="h-6 shrink-0 border-warning/40 bg-warning/10 text-warning hover:bg-warning/20"
        onclick={() =>
          experimentalStore.setEnabled("silenceDetection", true)}
      >
        Enable
      </Button>
    </div>
  {/if}

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
          href="/recasts"
          class="gap-1.5"
        >
          <ArrowLeft size={13} />
          Back to recordings
        </Button>
      </div>
    </div>
  {:else}
    <div class="flex min-h-0 flex-1 overflow-hidden">
      <!-- Preview + playback + timeline -->
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
              bind:captureFrame
              bind:webcodecsActive
              {videoSrc}
              {cursorPath}
              {cameraSrc}
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
            {captureFrame}
            bind:loopEnabled
            fullscreenTargetEl={previewContainerEl}
          />
        </div>

        <!-- `slide` (axis:y) animates the wrapper height to 0 while the inner
             keeps its height, so the preview reclaims space smoothly. -->
        {#if showTimeline}
          <div
            class="shrink-0 overflow-hidden"
            transition:slide={{ axis: "y", duration: 240, easing: cubicOut }}
          >
            <Timeline {store} {videoEl} {tileProvider} {filmstripVersion} />
          </div>
        {/if}
      </div>

      <!-- Inner div holds the fixed width so `slide` (axis:x) clips cleanly
           instead of reflowing the panel's container queries. -->
      {#if showSidebar}
        <aside
          class="min-h-0 shrink-0 overflow-hidden border-l border-border/60"
          transition:slide={{ axis: "x", duration: 240, easing: cubicOut }}
        >
          <div class="h-full w-80 xl:w-88">
            <PropertiesPanel {store} {cameraPath} />
          </div>
        </aside>
      {/if}
    </div>
  {/if}

  <!-- .recast stores system + mic audio as separate WAVs (the mp4 has no audio);
       kept in lockstep with the video via the $effects above. -->
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

  <ExportFlowDialog
    open={isExportFlowOpen}
    phase={exportPhase}
    onEscape={handleExportEscape}
    onBackdropClick={handleExportBackdrop}
    {options}
    {progress}
    {success}
    {cancelled}
    error={errorPanel}
  />
</div>

{#snippet options()}
  <ExportDialog
    {store}
    onConfirm={confirmExportOptions}
    onCancel={dismissExportOptions}
  />
{/snippet}

{#snippet exportSpecStrip()}
  {@const fmt = store.exportFormat}
  {@const isGifFmt = fmt === "gif"}
  {@const srcFps = Math.max(1, Math.round(store.metadata?.fps ?? 60))}
  {@const qualityLabel = isGifFmt
    ? store.gifSettings.quality === "low"
      ? "Lite"
      : store.gifSettings.quality === "high"
        ? "Vivid"
        : "Standard"
    : store.exportQuality === "small"
      ? "720p"
      : store.exportQuality === "hd"
        ? "1080p"
        : store.exportQuality === "4k"
          ? "2160p"
          : "Source"}
  {@const fpsLabel = isGifFmt
    ? store.gifSettings.fps
      ? `${store.gifSettings.fps} fps`
      : "Auto"
    : store.exportFps
      ? `${store.exportFps} fps`
      : `${srcFps} fps`}
  <!-- Carries the committed export settings forward so every later phase stays
       anchored to "what you're exporting". -->
  <section
    class="flex items-stretch divide-x divide-border/40 border-b border-border/40 bg-muted/15 px-5 py-2.5"
  >
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 pr-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >Format</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {fmt.toUpperCase()}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 px-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >{isGifFmt ? "Colors" : "Quality"}</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {qualityLabel}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 px-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >Frame rate</span
      >
      <span class="truncate text-[12px] font-medium tabular-nums text-foreground">
        {fpsLabel}
      </span>
    </div>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5 pl-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >Duration</span
      >
      <span class="truncate font-mono text-[12px] tabular-nums text-foreground">
        {formatTime(getExportDuration())}
      </span>
    </div>
  </section>
{/snippet}

{#snippet progress()}
  {@const isPreparing =
    !exportHasProgress && !exportFinalizing}
  {@const eta = exportEtaMs()}
  {@const ringPct = isPreparing
    ? 0
    : exportFinalizing
      ? 100
      : Math.min(100, Math.max(0, displayPct))}
  {@const RING_R = 52}

  <div class="flex flex-col" style="width: 540px;">
    <header
      class="flex items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
      >
        <Upload class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
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
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          {#if exportCancelling}
            Stopping and cleaning up the partial file…
          {:else if exportFinalizing}
            Writing the finished file to disk…
          {:else if isPreparing}
            Getting frames and effects ready…
          {:else}
            This can take a moment. You can keep working.
          {/if}
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <div
      class="mx-auto flex w-full max-w-xs flex-col items-center gap-3 px-5 pt-5 pb-3"
    >
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
                  <!-- Indeterminate spinner. `pathLength="100"` decouples the
                       dash math from 2πr so precision can't leave it short of full. -->
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
                  <!-- Dash values in inline style so they participate in the CSS
                       transition; mixing attribute + style breaks it in some engines. -->
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
              <!-- Percentage during encode; dashes while preparing/finalising. -->
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

            <!-- Rotating status line, shown only while frames are encoding. -->
            {#if !isPreparing && !exportFinalizing && !exportCancelling}
              <div class="relative h-4 self-stretch" aria-live="polite">
                {#key encodeMessageIndex}
                  <span
                    in:fade={{ duration: 320 }}
                    out:fade={{ duration: 320 }}
                    class="export-shimmer absolute inset-0 flex items-center justify-center text-[11px] font-medium tracking-tight"
                  >
                    {ENCODE_MESSAGES[encodeMessageIndex]}…
                  </span>
                {/key}
              </div>
            {/if}

            <!-- Substage list; collapses to a single "Encoding…" line once Rust takes over. -->
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
                      <!-- Dots travel through a pipe, suggesting the render state
                           being piped to the encoder. -->
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
                      <LoaderCircle
                        size={11}
                        class="shrink-0 animate-spin text-primary"
                      />
                      <span class="text-foreground">{s.label}</span>
                    {:else}
                      <Circle
                        size={11}
                        class="shrink-0 text-muted-foreground/40"
                      />
                      <span class="text-muted-foreground/60">{s.label}</span>
                    {/if}
                  </li>
                {/if}
              {/each}
            </ul>
          </div>

    <footer
      class="flex items-center justify-end gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button
        variant="destructive_soft"
        size="xs"
        class="gap-1.5"
        onclick={handleCancelExport}
        disabled={exportCancelling}
      >
        <X class="size-3" />
        {exportCancelling ? "Cancelling…" : "Cancel export"}
      </Button>
    </footer>
  </div>
{/snippet}

{#snippet success()}
  <div class="flex flex-col" style="width: 540px;">
    <header class="flex items-start gap-3 border-b border-border/40 px-5 py-4">
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-success/30 bg-success/10 text-success shadow-(--shadow-craft-inset)"
      >
        <CheckCircle2 class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
        >
          Export complete
        </h3>
        {#if exportResult?.kind === "success"}
          <p
            class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground"
            title={exportResult.path}
          >
            {exportResult.path}
          </p>
        {/if}
      </div>
    </header>

    {@render exportSpecStrip()}

    {#if successUpload}
      <!-- Drive status row with inline progress and a trailing action that
           tracks the upload state (cancel / copy-link / retry). -->
      <div
        class="flex items-center gap-3 border-t border-border/40 bg-muted/15 px-5 py-3"
        aria-live="polite"
      >
        <div
          class="flex size-7 shrink-0 items-center justify-center rounded-md border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset)"
        >
          {#if successUpload.status === "uploading"}
            <RefreshCw class="size-3.5 animate-spin text-primary" />
          {:else if successUpload.status === "complete"}
            <HardDriveUpload class="size-3.5 text-success" />
          {:else if successUpload.status === "cancelled"}
            <X class="size-3.5" />
          {:else}
            <TriangleAlert class="size-3.5 text-destructive" />
          {/if}
        </div>

        <div class="min-w-0 flex-1">
          <p class="text-[12px] font-medium text-foreground">
            {#if successUpload.status === "uploading"}
              Uploading to Drive
            {:else if successUpload.status === "complete"}
              Uploaded to Drive
            {:else if successUpload.status === "cancelled"}
              Upload cancelled
            {:else}
              Upload failed
            {/if}
          </p>
          {#if successUpload.status === "uploading"}
            <div class="mt-1 flex items-center gap-2">
              <div class="h-1 flex-1 overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full bg-primary transition-[width] duration-200"
                  style="width: {successUploadPct}%"
                ></div>
              </div>
              <span
                class="font-mono text-[10px] tabular-nums text-muted-foreground"
              >
                {successUploadPct}%
              </span>
            </div>
          {:else if successUpload.status === "error" && successUpload.error}
            <p
              class="truncate text-[10.5px] leading-snug text-muted-foreground"
              title={successUpload.error}
            >
              {successUpload.error}
            </p>
          {/if}
        </div>

        <!-- The Drive row owns its lifecycle so the footer carries no Drive action. -->
        {#if successUpload.status === "uploading"}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5 text-muted-foreground"
            onclick={() => gdrive.cancelUpload(successUpload!.uploadId)}
          >
            <X class="size-3" />
            Cancel
          </Button>
        {:else if successUpload.status === "complete" && successUpload.webViewLink}
          <div class="flex shrink-0 items-center gap-0.5">
            <Button
              variant="ghost"
              size="xs"
              class="gap-1.5 text-primary hover:text-primary"
              onclick={() => copyDriveLink(successUpload!.webViewLink!)}
            >
              <Link2 class="size-3" />
              Copy link
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground"
              title="Open in Drive"
              onclick={() => openDriveLink(successUpload!.webViewLink!)}
            >
              <ExternalLink class="size-3" />
            </Button>
          </div>
        {:else}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5 text-muted-foreground"
            onclick={uploadExportToDrive}
          >
            <RefreshCw class="size-3" />
            Retry
          </Button>
        {/if}
      </div>
    {/if}

    <!-- Share/upload tiles, grouped out of the footer so they read as one
         "where does this go?" choice. The Drive tile drops out once an upload
         exists — the Drive row above owns it then. -->
    <div class="border-t border-border/40 bg-muted/15 px-5 py-3.5">
      <p
        class="mb-2.5 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Share or upload
      </p>
      <div class="flex items-stretch gap-2">
        <button
          type="button"
          onclick={shareCurrentExportToCloud}
          class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
        >
          <span
            class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
          >
            <Cloud class="size-4" />
          </span>
          <span class="text-[11px] font-medium leading-none text-foreground">
            Recast Cloud
          </span>
        </button>

        {#if !successUpload}
          <button
            type="button"
            onclick={uploadExportToDrive}
            class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
          >
            <span
              class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
            >
              <HardDriveUpload class="size-4" />
            </span>
            <span class="text-[11px] font-medium leading-none text-foreground">
              Google Drive
            </span>
          </button>
        {/if}

        {#if shareSupported}
          <button
            type="button"
            onclick={shareExportedFile}
            title="Open the system share sheet"
            class="group/dest flex flex-1 flex-col items-center gap-2 rounded-lg border border-border/50 bg-card/60 px-3 py-3 text-center shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm"
          >
            <span
              class="flex size-8 items-center justify-center rounded-lg border border-border/50 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset) transition-colors group-hover/dest:text-primary"
            >
              <Share2 class="size-4" />
            </span>
            <span class="text-[11px] font-medium leading-none text-foreground">
              System share
            </span>
          </button>
        {/if}
      </div>
    </div>

    <footer
      class="flex items-center justify-between gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button
        variant="ghost"
        size="xs"
        class="gap-1.5 text-muted-foreground"
        onclick={dismissExportResult}
      >
        Dismiss
        <Kbd class="ml-0.5">Esc</Kbd>
      </Button>

      <Button
        variant="default"
        size="xs"
        class="gap-1.5"
        onclick={revealExportInFolder}
      >
        <FolderOpen class="size-3" />
        Show in folder
      </Button>
    </footer>
  </div>
{/snippet}

{#snippet cancelled()}
  <div class="flex flex-col" style="width: 540px;">
    <header
      class="flex items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-border/60 bg-muted text-muted-foreground shadow-(--shadow-craft-inset)"
      >
        <X class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
        >
          Export cancelled
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Stopped before finishing, so no file was written. Your settings are
          kept, so you can pick up right where you left off.
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <footer
      class="flex items-center justify-end gap-1.5 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button variant="ghost" size="xs" onclick={dismissExportResult}
        >Dismiss</Button
      >
      <Button
        variant="default"
        size="xs"
        class="gap-1.5"
        onclick={handleExport}
      >
        <Upload class="size-3" />
        Export again
      </Button>
    </footer>
  </div>
{/snippet}

{#snippet errorPanel()}
  <div class="flex flex-col" style="width: 540px;">
    <header
      class="flex items-start gap-3 border-b border-border/40 px-5 py-4"
    >
      <div
        class="flex size-10 shrink-0 items-center justify-center rounded-xl border border-destructive/30 bg-destructive/10 text-destructive shadow-(--shadow-craft-inset)"
      >
        <TriangleAlert class="size-4" />
      </div>
      <div class="min-w-0 flex-1 pt-0.5">
        <h3
          id="export-flow-title"
          class="text-[14px] font-semibold tracking-tight text-foreground"
        >
          Export failed
        </h3>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          Something went wrong while encoding. Your settings are kept, so try
          again, or adjust them first.
        </p>
      </div>
    </header>

    {@render exportSpecStrip()}

    <!-- Raw FFmpeg/pipeline message, scrollable so a long stack doesn't blow out
         the dialog height. -->
    <div
      class="max-h-40 overflow-y-auto border-b border-border/40 px-5 py-3"
    >
      <p
        class="mb-1.5 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Details
      </p>
      {#if exportResult?.kind === "error"}
        <pre
          class="whitespace-pre-wrap wrap-break-word font-mono text-[10px] leading-snug text-destructive">{exportResult.message}</pre>
      {/if}
    </div>
    <footer
      class="flex items-center justify-end gap-1.5 border-t border-border/40 bg-muted/30 px-3 py-2.5"
    >
      <Button variant="ghost" size="xs" onclick={dismissExportResult}
        >Dismiss</Button
      >
      <Button
        variant="default"
        size="xs"
        class="gap-1.5"
        onclick={handleExport}
      >
        <Upload class="size-3" />
        Try again
      </Button>
    </footer>
  </div>
{/snippet}

<style>
  /* Three dots travel left → right with offset, reading as data being piped. */
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
  /* Primary-tinted highlight sweeps across muted text for a subtle shimmer. */
  .export-shimmer {
    background: linear-gradient(
      100deg,
      var(--muted-foreground) 0%,
      var(--muted-foreground) 38%,
      var(--primary) 50%,
      var(--muted-foreground) 62%,
      var(--muted-foreground) 100%
    );
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: export-shimmer-sweep 2.4s linear infinite;
  }
  @keyframes export-shimmer-sweep {
    from {
      background-position: 160% 0;
    }
    to {
      background-position: -160% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .export-shimmer {
      animation: none;
      background-position: 50% 0;
    }
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
