<script lang="ts">
  import {
    Circle,
    FlipHorizontal2,
    LoaderCircle,
    Maximize2,
    Square,
    Squircle,
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
    setWindowAspectRatio,
    updateCameraPreviewState,
    validateCameraSource,
  } from "$lib/ipc";
  import { isBrowserDeviceId } from "$lib/runtime/device-id";
  import {
    allowedShapesFor,
    ASPECT_RATIO,
    ASPECTS,
    buildPreviewState,
    computeSizeConstraints,
    CONTROL_BAR_HEIGHT,
    fitInsideMax,
    MAX_SCREEN_FRACTION,
    MIN_LOGICAL_SIZE,
    targetWindowSize,
    WINDOW_RADIUS,
    type AspectKey,
    type CameraStatus,
    type ShapeKey,
  } from "./camera-preview.logic";

  // Cached max logical size; aspect-snap helpers clamp against it because the
  // OS max-size only bounds drag-resize, not our programmatic setSize calls.
  let maxLogicalW = $state(640);
  let maxLogicalH = $state(360);

  let videoEl: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = $state(null);
  let errorMessage: string | null = $state(null);
  let statusMessage = $state("Connecting to camera…");
  let status = $state<CameraStatus>("loading");
  let isMirrored = $state(true);
  let aspect = $state<AspectKey>("1:1");
  let shape = $state<ShapeKey>("rounded");
  let liveProbeTimer: number | null = $state(null);
  let videoFrameSeen = $state(false);
  let isSnapping = false;

  const params = new URLSearchParams(window.location.search);
  // Accepts both legacy DirectShow names and browser MediaDevices ids.
  const deviceQuery = params.get("deviceId");

  $effect(() => {
    if (videoEl && stream) {
      videoEl.srcObject = stream;
    }
  });

  onMount(() => {
    // Make the WebView see-through so only the inner rounded container paints;
    // the OS window is already transparent, so corners show the desktop.
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

    // Push preview state only on actual window changes, not on a poll
    // (the old 350ms poll hit a Rust mutex thrice a second even when idle).
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

      // Validation only applies to DirectShow names; skip browser deviceId hashes.
      if (deviceQuery && !isBrowserDeviceId(deviceQuery)) {
        try {
          const validation = await validateCameraSource(deviceQuery);
          if (validation.status === "warning" || validation.status === "error") {
            status = validation.status === "error" ? "failed" : "warning";
            statusMessage =
              validation.statusMessage ?? "Camera source requires validation.";
          }
        } catch {
          // Non-fatal. Preview can still open via browser enumeration.
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

  // Apply OS min/max size constraints. Cap is keyed off screen width; every
  // aspect is landscape-or-square (ratio ≥ 1) so a square max box bounds the
  // window by width without clipping the proportional height.
  async function applySizeConstraints() {
    const { maxLogicalW: maxW, maxLogicalH: maxH, minLogicalW, minWinH } =
      computeSizeConstraints(window.screen.availWidth || 1920);
    // Square video bounding box; the window adds the control strip on top.
    maxLogicalW = maxW;
    maxLogicalH = maxH;

    const win = getCurrentWindow();
    try {
      await win.setMinSize(new LogicalSize(minLogicalW, minWinH));
      await win.setMaxSize(
        new LogicalSize(maxLogicalW, maxLogicalH + CONTROL_BAR_HEIGHT),
      );
    } catch (e) {
      console.warn("camera preview size constraints failed:", e);
    }

    // Install (or refresh) the native aspect lock for the current aspect.
    void applyNativeAspectLock();
  }

  // Hand the aspect ratio to the Windows-native WM_SIZING constraint so drag
  // resizes proportionally. No-op off Windows, where `snapToAspect` is the
  // fallback. The drag rect is in physical pixels, so the min crosses as such.
  async function applyNativeAspectLock() {
    try {
      const ratio = ASPECT_RATIO[aspect];
      const dpr = window.devicePixelRatio || 1;
      await setWindowAspectRatio(
        "camera-preview",
        ratio,
        1,
        MAX_SCREEN_FRACTION,
        Math.round(MIN_LOGICAL_SIZE * dpr),
        Math.round(CONTROL_BAR_HEIGHT * dpr),
      );
    } catch (e) {
      // Non-Windows / older build. The JS snap-to-aspect path still applies.
      console.warn("native aspect lock unavailable:", e);
    }
  }

  async function applyAspect(
    next: AspectKey,
    opts: { snap?: boolean } = {},
  ) {
    aspect = next;
    // Re-sync the native ratio so the next drag uses the new aspect.
    void applyNativeAspectLock();
    if (opts.snap) {
      const win = getCurrentWindow();
      const size = await win.outerSize();
      const factor = window.devicePixelRatio || 1;
      // Window width == video width (no horizontal chrome).
      const widthLogical = size.width / factor;
      const ratio = ASPECT_RATIO[next];
      const [clampedW, clampedVideoH] = targetWindowSize(
        widthLogical,
        ratio,
        maxLogicalW,
        maxLogicalH,
      );
      isSnapping = true;
      // Window height = video height + control strip.
      await win.setSize(
        new LogicalSize(clampedW, clampedVideoH + CONTROL_BAR_HEIGHT),
      );
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
    // Drag deltas arrive as *window* dimensions, so peel off the control strip to
    // get the video box the aspect ratio actually governs.
    const videoH = physHeight / factor - CONTROL_BAR_HEIGHT;
    const target = ASPECT_RATIO[aspect];
    const expectedVideoH = w / target;
    const [clampedW, clampedVideoH] = fitInsideMax(
      w,
      expectedVideoH,
      target,
      maxLogicalW,
      maxLogicalH,
    );
    if (
      Math.abs(clampedVideoH - videoH) <= 1 &&
      Math.abs(clampedW - w) <= 1
    ) return;
    isSnapping = true;
    try {
      await getCurrentWindow().setSize(
        new LogicalSize(clampedW, clampedVideoH + CONTROL_BAR_HEIGHT),
      );
    } finally {
      window.setTimeout(() => {
        isSnapping = false;
      }, 50);
    }
  }

  function cycleAspect() {
    const nextIndex = (ASPECTS.indexOf(aspect) + 1) % ASPECTS.length;
    const next = ASPECTS[nextIndex];
    // Circle → rounded off 1:1; a circle on a non-square box renders as an
    // ellipse the editor's composited bubble doesn't support.
    if (next !== "1:1" && shape === "circle") {
      shape = "rounded";
    }
    void applyAspect(next, { snap: true });
  }

  function cycleShape() {
    const allowed = allowedShapesFor(aspect);
    const idx = allowed.indexOf(shape);
    // Start from the first allowed option if the current shape isn't allowed.
    shape = allowed[(idx === -1 ? 0 : idx + 1) % allowed.length];
    void reportPreviewState();
  }

  function toggleMirror() {
    isMirrored = !isMirrored;
    void reportPreviewState();
  }

  // `circle` is 50% (box is always 1:1 then), `rounded` matches the token.
  const cssRadius = $derived.by(() => {
    switch (shape) {
      case "circle":
        return "50%";
      case "square":
        return "0px";
      default:
        return `${WINDOW_RADIUS}px`;
    }
  });

  // Icon + tooltip for the current shape, drives the cycle button's label.
  const shapeMeta = $derived.by(() => {
    switch (shape) {
      case "circle":
        return { icon: Circle, label: "Circle" };
      case "square":
        return { icon: Square, label: "Square" };
      default:
        return { icon: Squircle, label: "Rounded" };
    }
  });

  async function reportPreviewState() {
    const win = getCurrentWindow();
    const position = await win.outerPosition();
    const size = await win.outerSize();
    const state = buildPreviewState(
      position,
      size,
      {
        width: Math.max(window.screen.availWidth || 1, 1),
        height: Math.max(window.screen.availHeight || 1, 1),
      },
      window.devicePixelRatio || 1,
      shape,
      isMirrored,
      status,
    );
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
  class="group/root relative flex h-screen w-full select-none flex-col scroll-m-0 scrollbar-none"
  
>
  <!-- Video bubble, the only clipped/rounded surface; `flex-1` is the window
       height minus the control strip, the region the aspect lock governs. -->
  <div
    class="relative min-h-0 w-full flex-1 overflow-hidden bg-card transition-[border-radius] duration-150 ease-out motion-reduce:transition-none"
    data-tauri-drag-region
    style="border-radius: {cssRadius}"
  >
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={videoEl}
      autoplay
      playsinline
      muted
      class="pointer-events-none h-full w-full object-cover"
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
  </div>

  <!-- Control strip below the bubble (outside its overflow) so the pill is
       never clipped; fades in on hover. -->
  <div
    class="flex w-full shrink-0 items-center justify-center"
    
    style="height: {CONTROL_BAR_HEIGHT}px"
  >
    <div
      class="pointer-events-none flex items-center gap-1 rounded-full border border-border-subtle bg-background/78 px-1 py-1 opacity-0 shadow-craft-floating backdrop-blur-3xl transition-opacity duration-200 group-hover/root:pointer-events-auto group-hover/root:opacity-100"
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

      {#snippet shapeIcon()}
        {@const SIcon = shapeMeta.icon}
        <SIcon size={11} strokeWidth={2} />
      {/snippet}
      <Button
        onclick={cycleShape}
        onmousedown={(e: MouseEvent) => e.stopPropagation()}
        variant="ghost"
        size="icon-sm"
        class="size-6 rounded-full"
        title={aspect === "1:1"
          ? `Cycle shape: square → rounded → circle (now ${shapeMeta.label})`
          : `Cycle shape: square ↔ rounded (now ${shapeMeta.label})`}
      >
        {@render shapeIcon()}
      </Button>

      <Button
        onclick={toggleMirror}
        onmousedown={(e: MouseEvent) => e.stopPropagation()}
        variant={isMirrored ? "default_soft" : "ghost"}
        size="icon-sm"
        class="size-6 rounded-full"
        title={isMirrored ? "Mirror: on (flip horizontally)" : "Mirror: off"}
      >
        <FlipHorizontal2 size={12} strokeWidth={2} />
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
</div>

<style>
  /* Hide the scrollbar + gutter for this page only so the rounded corners read
     through to the desktop (the global stylesheet sets scrollbar-gutter: stable). */
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
