<script lang="ts">
  import { Circle, RotateCcw, Square, X } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let videoEl: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = $state(null);
  let errorMessage: string | null = $state(null);
  let isMirrored = $state(true);
  let isCircle = $state(false);

  const borderRadius = $derived(isCircle ? 999 : 16);

  // deviceId here is the FFmpeg/DirectShow friendly name (e.g. "Integrated Camera"),
  // not a browser deviceId — we resolve it via enumerateDevices() labels below.
  const params = new URLSearchParams(window.location.search);
  const deviceName = params.get("deviceId");

  $effect(() => {
    if (videoEl && stream) {
      videoEl.srcObject = stream;
    }
  });

  onMount(() => {
    startCamera();

    const unlistenPromise = listen("camera-stop", () => {
      stopCamera();
      getCurrentWindow().close();
    });

    return () => {
      stopCamera();
      unlistenPromise.then((fn) => fn());
    };
  });

  async function resolveBrowserDeviceId(name: string): Promise<string | null> {
    let devices = await navigator.mediaDevices.enumerateDevices();
    let labelsPopulated = devices.some(
      (d) => d.kind === "videoinput" && d.label,
    );

    // Labels are empty until the page has been granted camera access at least
    // once. Prime the permission with a throwaway stream, then re-enumerate.
    if (!labelsPopulated) {
      const probe = await navigator.mediaDevices.getUserMedia({ video: true });
      probe.getTracks().forEach((t) => t.stop());
      devices = await navigator.mediaDevices.enumerateDevices();
    }

    const match = devices.find(
      (d) =>
        d.kind === "videoinput" &&
        (d.label === name || d.label.includes(name)),
    );
    return match?.deviceId ?? null;
  }

  async function startCamera() {
    try {
      errorMessage = null;
      let videoConstraints: MediaTrackConstraints | true = true;

      if (deviceName) {
        const browserId = await resolveBrowserDeviceId(deviceName);
        if (browserId) {
          videoConstraints = { deviceId: { ideal: browserId } };
        }
      }

      stream = await navigator.mediaDevices.getUserMedia({
        video: videoConstraints,
        audio: false,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Camera access failed:", e);
      errorMessage = msg;
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
  }

  function toggleMirror() {
    isMirrored = !isMirrored;
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
  class="group/root relative h-screen w-screen overflow-hidden bg-background select-none"
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
    style="border-radius: {borderRadius}px; transform: {isMirrored
      ? 'scaleX(-1)'
      : 'none'}"
  ></video>

  {#if errorMessage}
    <div
      class="absolute inset-0 flex items-center justify-center p-4 text-center bg-background/80 backdrop-blur-sm pointer-events-none"
      style="border-radius: {borderRadius}px"
    >
      <p class="text-[11px] font-medium text-foreground leading-relaxed">
        Camera unavailable<br />
        <span class="text-muted-foreground">{errorMessage}</span>
      </p>
    </div>
  {/if}

  <!-- Floating controls overlay — auto-reveal on hover, matches panel.svelte chrome. -->
  <div
    class="pointer-events-none absolute bottom-2 left-1/2 -translate-x-1/2 flex items-center gap-0.5 rounded-full border border-border-subtle bg-background/70 backdrop-blur-3xl px-1 py-1 shadow-craft-floating opacity-0 transition-opacity duration-200 group-hover/root:opacity-100 group-hover/root:pointer-events-auto"
  >
    <Button
      onclick={toggleShape}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="icon-sm"
      class="size-6 rounded-full"
      title={isCircle ? "Rounded rectangle" : "Circle"}
    >
      {#if isCircle}
        <Square size={11} strokeWidth={2} />
      {:else}
        <Circle size={11} strokeWidth={2} />
      {/if}
    </Button>

    <Button
      onclick={toggleMirror}
      onmousedown={(e) => e.stopPropagation()}
      variant={isMirrored ? "default_soft" : "ghost"}
      size="icon-sm"
      class="size-6 rounded-full"
      title={isMirrored ? "Unmirror" : "Mirror"}
    >
      <RotateCcw size={11} strokeWidth={2} />
    </Button>

    <div class="mx-0.5 h-3 w-px bg-border"></div>

    <Button
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      variant="destructive_soft"
      size="icon-sm"
      class="size-6 rounded-full"
      title="Close camera (Esc)"
    >
      <X size={11} strokeWidth={2.5} />
    </Button>
  </div>
</div>
