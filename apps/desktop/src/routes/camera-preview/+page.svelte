<script lang="ts">
  import {
    LoaderCircle,
    Maximize2,
    RotateCcw,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { listen } from "@tauri-apps/api/event";
  import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  import {
    CameraNotFoundError,
    openCameraStream,
  } from "$lib/camera/browser-devices";
  import {
    updateCameraPreviewState,
    validateCameraSource,
    type CameraPreviewState,
  } from "$lib/ipc";

  type AspectKey = "1:1" | "4:3" | "16:9";
  type CameraStatus = "loading" | "live" | "warning" | "failed";

  const ASPECTS: AspectKey[] = ["1:1", "4:3", "16:9"];
  const ASPECT_RATIO: Record<AspectKey, number> = {
    "1:1": 1,
    "4:3": 4 / 3,
    "16:9": 16 / 9,
  };

  // Window radius in CSS pixels — matches the rounded-3xl token visually.
  const WINDOW_RADIUS = 20;

  // Resize bounds, expressed as fractions of the primary screen's available
  // logical dimensions. The cap exists because an oversized live preview
  // (a) covers content the user is recording, and (b) makes the eventual
  // composited bubble look ridiculous if the user accidentally records at
  // that size. 25% of either axis is comfortably visible without dominating
  // the frame.
  const MAX_SCREEN_FRACTION = 0.25;
  // Floor: small enough to tuck into a corner, large enough that the live
  // feed remains intelligible. ~120 logical px ≈ a thumbnail.
  const MIN_LOGICAL_SIZE = 120;

  // Cached max logical size for the current screen — recomputed on mount and
  // whenever a resize crosses screens. Used by the aspect-snap helpers to
  // clamp the box before calling setSize, since the OS-level max-size only
  // bounds drag-resize and not our programmatic snap-to-aspect calls.
  let maxLogicalW = $state(640);
  let maxLogicalH = $state(360);

  let videoEl: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = $state(null);
  let errorMessage: string | null = $state(null);
  let statusMessage = $state("Connecting to camera…");
  let status = $state<CameraStatus>("loading");
  let isMirrored = $state(true);
  let aspect = $state<AspectKey>("1:1");
  let liveProbeTimer: number | null = $state(null);
  let videoFrameSeen = $state(false);
  let isSnapping = false;

  const params = new URLSearchParams(window.location.search);
  // Accept both legacy DirectShow names (passed as `deviceId` historically)
  // and real browser MediaDevices ids — `openCameraStream` handles either.
  const deviceQuery = params.get("deviceId");

  $effect(() => {
    if (videoEl && stream) {
      videoEl.srcObject = stream;
    }
  });

  onMount(() => {
    // Make the WebView itself fully see-through so the inner rounded
    // container is the only thing that paints — the OS window already has
    // `transparent: true`, so corners outside the radius show the desktop.
    const html = document.documentElement;
    const body = document.body;
    html.style.background = "transparent";
    html.style.overflow = "hidden";
    html.style.scrollbarGutter = "auto";
    (html.style as CSSStyleDeclaration & { scrollbarWidth?: string }).scrollbarWidth =
      "none";
    body.style.background = "transparent";
    body.style.overflow = "hidden";
    body.style.margin = "0";

    void applySizeConstraints();
    void startCamera();
    void applyAspect(aspect, { snap: true });

    const unlistenStop = listen("camera-stop", () => {
      stopCamera();
      getCurrentWindow().close();
    });
    const unlistenStarted = listen<{ startedAtUnixMs: number }>(
      "camera-recording-started",
      () => {
        void reportPreviewState();
      },
    );
    const unlistenStopped = listen("camera-recording-stopped", () => {});

    // Event-driven preview state push: only fire on actual window changes
    // (move/resize) or user toggles (aspect/mirror). Replaces the old 350ms
    // poll that did three IPC round-trips/sec into a Rust mutex even when
    // nothing changed.
    const unlistenResize = getCurrentWindow().onResized(({ payload }) => {
      void snapToAspect(payload.width, payload.height);
      void reportPreviewState();
    });
    const unlistenMove = getCurrentWindow().onMoved(() => {
      void reportPreviewState();
    });

    return () => {
      stopCamera();
      if (liveProbeTimer !== null) window.clearTimeout(liveProbeTimer);
      unlistenStop.then((fn) => fn());
      unlistenStarted.then((fn) => fn());
      unlistenStopped.then((fn) => fn());
      unlistenResize.then((fn) => fn());
      unlistenMove.then((fn) => fn());
    };
  });

  async function startCamera() {
    try {
      errorMessage = null;
      status = "loading";
      statusMessage = "Connecting to camera…";

      // Validation is best-effort and only meaningful for DirectShow names;
      // skip silently if the query is a browser deviceId hash (validation
      // would always fail on those).
      if (deviceQuery && !/^[a-f0-9]{40,}$/i.test(deviceQuery)) {
        try {
          const validation = await validateCameraSource(deviceQuery);
          if (validation.status === "warning" || validation.status === "error") {
            status = validation.status === "error" ? "failed" : "warning";
            statusMessage =
              validation.statusMessage ?? "Camera source requires validation.";
          }
        } catch {
          // Non-fatal — preview can still open via browser enumeration.
        }
      }

      const { stream: openedStream, camera } = await openCameraStream(
        deviceQuery,
      );
      stream = openedStream;
      console.info(
        `[camera-preview] opened ${camera.label} (virtual=${camera.isVirtual})`,
      );

      startLivelinessProbe();
      window.setTimeout(() => {
        void reportPreviewState();
      }, 150);
    } catch (e) {
      const msg =
        e instanceof CameraNotFoundError
          ? e.message
          : e instanceof Error
            ? e.message
            : String(e);
      console.error("Camera access failed:", e);
      errorMessage = msg;
      status = "failed";
      statusMessage = msg;
    }
  }

  function startLivelinessProbe() {
    videoFrameSeen = false;

    const markLive = () => {
      if (!videoEl) return;
      if (videoEl.videoWidth > 0 && videoEl.videoHeight > 0) {
        videoFrameSeen = true;
        if (status !== "failed") {
          status = "live";
          statusMessage = "Camera live";
        }
      }
    };

    const interval = window.setInterval(() => {
      markLive();
      if (videoFrameSeen) window.clearInterval(interval);
    }, 150);

    liveProbeTimer = window.setTimeout(() => {
      window.clearInterval(interval);
      if (!videoFrameSeen && status !== "failed") {
        status = "warning";
        statusMessage = "Camera opened but no live frames arrived.";
      }
    }, 2200);
  }

  function stopCamera() {
    if (stream) {
      stream.getTracks().forEach((t) => t.stop());
      stream = null;
    }
  }

  function closeWindow() {
    stopCamera();
    getCurrentWindow().close();
  }

  /**
   * Compute and apply OS-level min/max size constraints based on the
   * primary screen's available logical dimensions. Called once on mount.
   * The OS handles drag-resize clamping for free once these are set; the
   * aspect-snap helpers do their own arithmetic clamp on top so that
   * programmatic setSize calls (cycling aspect) can't punch past the cap.
   */
  async function applySizeConstraints() {
    const screenW = Math.max(window.screen.availWidth || 1920, 320);
    const screenH = Math.max(window.screen.availHeight || 1080, 320);
    maxLogicalW = Math.floor(screenW * MAX_SCREEN_FRACTION);
    maxLogicalH = Math.floor(screenH * MAX_SCREEN_FRACTION);

    const win = getCurrentWindow();
    try {
      await win.setMinSize(new LogicalSize(MIN_LOGICAL_SIZE, MIN_LOGICAL_SIZE));
      await win.setMaxSize(new LogicalSize(maxLogicalW, maxLogicalH));
    } catch (e) {
      console.warn("camera preview size constraints failed:", e);
    }
  }

  /**
   * Fit a box of (w, h) with the given width/height ratio inside
   * (maxLogicalW, maxLogicalH) without breaking the ratio. Returns the
   * largest box that satisfies both bounds. Used by both `applyAspect`
   * (cycling aspect) and `snapToAspect` (height-snap after drag-resize)
   * since either path can derive a height that exceeds maxH on tall
   * aspects, or a width that exceeds maxW after a height-only drag.
   */
  function fitInsideMax(w: number, h: number, ratio: number): [number, number] {
    let outW = w;
    let outH = h;
    if (outW > maxLogicalW) {
      outW = maxLogicalW;
      outH = outW / ratio;
    }
    if (outH > maxLogicalH) {
      outH = maxLogicalH;
      outW = outH * ratio;
    }
    return [Math.round(outW), Math.round(outH)];
  }

  async function applyAspect(
    next: AspectKey,
    opts: { snap?: boolean } = {},
  ) {
    aspect = next;
    if (opts.snap) {
      const win = getCurrentWindow();
      const size = await win.outerSize();
      const factor = window.devicePixelRatio || 1;
      const widthLogical = size.width / factor;
      const ratio = ASPECT_RATIO[next];
      const [clampedW, clampedH] = fitInsideMax(
        widthLogical,
        widthLogical / ratio,
        ratio,
      );
      isSnapping = true;
      await win.setSize(new LogicalSize(clampedW, clampedH));
      window.setTimeout(() => {
        isSnapping = false;
      }, 50);
    }
    void reportPreviewState();
  }

  async function snapToAspect(physWidth: number, physHeight: number) {
    if (isSnapping) return;
    const factor = window.devicePixelRatio || 1;
    const w = physWidth / factor;
    const h = physHeight / factor;
    const target = ASPECT_RATIO[aspect];
    const expectedH = w / target;
    const [clampedW, clampedH] = fitInsideMax(w, expectedH, target);
    if (
      Math.abs(clampedH - h) <= 1 &&
      Math.abs(clampedW - w) <= 1
    ) return;
    isSnapping = true;
    try {
      await getCurrentWindow().setSize(
        new LogicalSize(clampedW, clampedH),
      );
    } finally {
      window.setTimeout(() => {
        isSnapping = false;
      }, 50);
    }
  }

  function cycleAspect() {
    const nextIndex = (ASPECTS.indexOf(aspect) + 1) % ASPECTS.length;
    void applyAspect(ASPECTS[nextIndex], { snap: true });
  }

  function toggleMirror() {
    isMirrored = !isMirrored;
    void reportPreviewState();
  }

  async function reportPreviewState() {
    const win = getCurrentWindow();
    const position = await win.outerPosition();
    const size = await win.outerSize();
    const screenWidth = Math.max(window.screen.availWidth || 1, 1);
    const screenHeight = Math.max(window.screen.availHeight || 1, 1);

    const factor = window.devicePixelRatio || 1;
    const widthLogical = size.width / factor;
    // Relative corner radius proportional to shorter side, capped sensibly.
    const shortLogical = Math.min(widthLogical, size.height / factor);
    const cornerRadius = Math.min(0.5, WINDOW_RADIUS / Math.max(shortLogical, 1));

    const state: CameraPreviewState = {
      mirror: isMirrored,
      shape: "rounded",
      cornerRadius,
      animationPreset: status === "warning" ? "lively" : "soft",
      windowX: Math.max(0, Math.min(1, position.x / screenWidth)),
      windowY: Math.max(0, Math.min(1, position.y / screenHeight)),
      windowWidth: Math.max(0.05, Math.min(1, size.width / screenWidth)),
      windowHeight: Math.max(0.05, Math.min(1, size.height / screenHeight)),
    };

    await updateCameraPreviewState(state);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeWindow();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="group/root relative h-screen w-full min-w-dvw select-none overflow-hidden bg-card scroll-m-0 scrollbar-none"
  data-tauri-drag-region
  style="border-radius: {WINDOW_RADIUS}px"
>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={videoEl}
    autoplay
    playsinline
    muted
    class="pointer-events-none h-full w-full min-w-dvw object-cover"
    style="transform: {isMirrored ? 'scaleX(-1)' : 'none'}"
  ></video>

  {#if status !== "live" || errorMessage}
    <div
      class="absolute inset-0 flex items-center justify-center bg-background/85 p-4 text-center backdrop-blur-md"
    >
      <div class="space-y-2">
        {#if status === "loading"}
          <LoaderCircle size={18} class="mx-auto animate-spin text-muted-foreground" />
        {/if}
        <p class="text-[11px] font-semibold text-foreground">
          {status === "failed" ? "Camera unavailable" : "Camera"}
        </p>
        <p class="max-w-[16rem] text-[10px] leading-relaxed text-muted-foreground">
          {errorMessage ?? statusMessage}
        </p>
      </div>
    </div>
  {/if}

  <div
    class="pointer-events-none absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-full border border-border-subtle bg-background/78 px-1 py-1 opacity-0 shadow-craft-floating backdrop-blur-3xl transition-opacity duration-200 group-hover/root:pointer-events-auto group-hover/root:opacity-100"
  >
    <Button
      onclick={cycleAspect}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant="ghost"
      size="sm"
      class="h-6 gap-1 rounded-full px-1.5 font-mono text-[10px] tabular-nums"
      title="Cycle aspect ratio"
    >
      <Maximize2 size={10} strokeWidth={2} />
      <span>{aspect}</span>
    </Button>

    <Button
      onclick={toggleMirror}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant={isMirrored ? "default_soft" : "ghost"}
      size="icon-sm"
      class="size-6 rounded-full"
      title={isMirrored ? "Unmirror camera" : "Mirror camera"}
    >
      <RotateCcw size={11} strokeWidth={2} />
    </Button>

    <div class="mx-0.5 h-3 w-px bg-border"></div>

    <Button
      onclick={closeWindow}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant="destructive_soft"
      size="icon-sm"
      class="size-6 rounded-full"
      title="Close camera (Esc)"
    >
      <X size={11} strokeWidth={2.5} />
    </Button>
  </div>
</div>

<style>
  /* Force-hide the scrollbar (and its reserved gutter) only for this page so
     the rounded corners read through to the desktop without a Windows
     scrollbar slot. The global stylesheet sets `scrollbar-gutter: stable`. */
  :global(html) {
    background: transparent !important;
    scrollbar-width: none;
    scrollbar-gutter: auto !important;
    overflow: hidden;
  }
  :global(body) {
    background: transparent !important;
    overflow: hidden;
    margin: 0;
  }
  :global(html::-webkit-scrollbar),
  :global(body::-webkit-scrollbar) {
    width: 0;
    height: 0;
    display: none;
  }
</style>
