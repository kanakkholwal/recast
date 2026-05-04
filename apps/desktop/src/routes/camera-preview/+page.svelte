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

  let videoEl: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = $state(null);
  let errorMessage: string | null = $state(null);
  let statusMessage = $state("Connecting to camera…");
  let status = $state<CameraStatus>("loading");
  let isMirrored = $state(true);
  let aspect = $state<AspectKey>("1:1");
  let reportTimer: number | null = $state(null);
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

    void startCamera();
    void applyAspect(aspect, { snap: true });

    reportTimer = window.setInterval(() => {
      void reportPreviewState();
    }, 350);

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

    const unlistenResize = getCurrentWindow().onResized(({ payload }) => {
      void snapToAspect(payload.width, payload.height);
    });

    return () => {
      stopCamera();
      if (reportTimer !== null) window.clearInterval(reportTimer);
      if (liveProbeTimer !== null) window.clearTimeout(liveProbeTimer);
      unlistenStop.then((fn) => fn());
      unlistenStarted.then((fn) => fn());
      unlistenStopped.then((fn) => fn());
      unlistenResize.then((fn) => fn());
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
      const heightLogical = Math.round(widthLogical / ASPECT_RATIO[next]);
      isSnapping = true;
      await win.setSize(new LogicalSize(Math.round(widthLogical), heightLogical));
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
    const expectedH = Math.round(w / target);
    if (Math.abs(expectedH - h) <= 1) return;
    isSnapping = true;
    try {
      await getCurrentWindow().setSize(
        new LogicalSize(Math.round(w), expectedH),
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
  class="group/root relative h-screen w-full min-w-dvw select-none overflow-hidden bg-transparent scroll-m-0 scrollbar-none"
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
