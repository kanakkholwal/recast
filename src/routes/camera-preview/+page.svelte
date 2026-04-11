<script lang="ts">
  import {
    Circle,
    RotateCcw,
    X
  } from "@lucide/svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let videoEl: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = $state(null);
  let borderRadius = $state(16);
  let isMirrored = $state(true);
  let isCircle = $state(false);
  let showControls = $state(true);
  let controlsTimer: ReturnType<typeof setTimeout> | null = null;

  // Get camera device from URL query
  const params = new URLSearchParams(window.location.search);
  const deviceId = params.get("deviceId");

  onMount(() => {
    startCamera();

    const unlistenPromise = listen("camera-stop", () => {
      stopCamera();
      getCurrentWindow().close();
    });

    resetControlsTimer();

    return () => {
      stopCamera();
      unlistenPromise.then((fn) => fn());
    };
  });

  async function startCamera() {
    try {
      const constraints: MediaStreamConstraints = {
        video: deviceId
          ? { deviceId: { exact: deviceId } }
          : true,
        audio: false,
      };
      stream = await navigator.mediaDevices.getUserMedia(constraints);
      if (videoEl) {
        videoEl.srcObject = stream;
      }
    } catch (e) {
      console.error("Camera access failed:", e);
    }
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

  function toggleShape() {
    isCircle = !isCircle;
    if (isCircle) {
      borderRadius = 999;
    } else {
      borderRadius = 16;
    }
  }

  function toggleMirror() {
    isMirrored = !isMirrored;
  }

  function handleMouseMove() {
    showControls = true;
    resetControlsTimer();
  }

  function resetControlsTimer() {
    if (controlsTimer) clearTimeout(controlsTimer);
    controlsTimer = setTimeout(() => {
      showControls = false;
    }, 3000);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="h-screen w-screen overflow-hidden bg-background select-none"
  onmousemove={handleMouseMove}
  data-tauri-drag-region
  style="border-radius: {borderRadius}px"
>
  <!-- Camera feed -->
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={videoEl}
    autoplay
    playsinline
    muted
    class="h-full w-full object-cover"
    style="border-radius: {borderRadius}px; transform: {isMirrored ? 'scaleX(-1)' : 'none'}"
  ></video>

  <!-- Floating controls overlay -->
  {#if showControls}
    <div
      class="absolute bottom-2 left-1/2 -translate-x-1/2 flex items-center gap-1 rounded-full bg-black/60 backdrop-blur-xl px-2 py-1 transition-opacity duration-300"
    >
      <!-- Border radius control -->
      <button
        onclick={toggleShape}
        onmousedown={(e) => e.stopPropagation()}
        class="size-7 rounded-full flex items-center justify-center text--card-foreground/60 hover:text--card-foreground hover:bg-card/10 transition-colors"
        title={isCircle ? "Rounded rectangle" : "Circle"}
      >
        <Circle size={13} strokeWidth={2} />
      </button>

      <!-- Mirror toggle -->
      <button
        onclick={toggleMirror}
        onmousedown={(e) => e.stopPropagation()}
        class="size-7 rounded-full flex items-center justify-center text-card-foreground/60 hover:text-card-foreground hover:bg-card/10 transition-colors"
        title={isMirrored ? "Unmirror" : "Mirror"}
      >
        <RotateCcw size={13} strokeWidth={2} />
      </button>

      <!-- Divider -->
      <div class="w-px h-3 bg-white/15 mx-0.5"></div>

      <!-- Close -->
      <button
        onclick={closeWindow}
        onmousedown={(e) => e.stopPropagation()}
        class="size-7 rounded-full flex items-center justify-center text-card-foreground/40 hover:text-red-400 hover:bg-card/10 transition-colors"
        title="Close camera"
      >
        <X size={13} strokeWidth={2} />
      </button>
    </div>
  {/if}
</div>
