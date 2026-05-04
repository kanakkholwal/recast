<script lang="ts">
  import { enumerateCameras } from "$lib/camera/browser-devices";
  import {
    getAudioDevices,
    getDisplays,
    getLastSource,
    setLastSource,
    startRecording,
    stopRecording,
    validateCameraSource,
    type CameraValidationResult,
    type RecordingOptions,
  } from "$lib/ipc";
  import {
    AppWindow,
    Camera,
    CameraOff,
    ChevronDown,
    Circle,
    Crop,
    Mic,
    MicOff,
    Monitor,
    Square,
    Volume2,
    VolumeOff,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { ButtonGroup } from "@recast/ui/button-group";
  import { emit, listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  type TargetSource = {
    type: "monitor" | "window" | "region";
    id: number;
    label: string;
    region?: {
      x: number;
      y: number;
      width: number;
      height: number;
    };
  };

  let selectedSource: TargetSource | null = $state(null);
  let isRecording = $state(false);
  let recordingStartTime: number | null = $state(null);
  let now = $state(Date.now());

  // Device toggles
  let systemAudioOn = $state(true);
  let micOn = $state(false);
  let cameraOn = $state(false);

  // Selected devices
  let selectedMicId = $state<string | null>(null);
  let selectedMicName = $state("Default");
  let selectedCameraId = $state<string | null>(null);
  let selectedCameraName = $state("Default");
  let cameraValidation = $state<CameraValidationResult | null>(null);

  async function refreshCameraValidation(deviceId: string | null) {
    if (!deviceId) {
      cameraValidation = null;
      return;
    }

    // Browser MediaDevices ids are 64-char hex hashes; the Rust validator
    // looks them up against DirectShow names and will always miss those.
    // Skip validation in that case — `openCameraStream` itself is the source
    // of truth for whether a browser-id camera will actually open.
    if (/^[a-f0-9]{40,}$/i.test(deviceId)) {
      cameraValidation = {
        id: deviceId,
        name: selectedCameraName,
        status: "ready",
        statusMessage: null,
        probedAtUnixMs: Date.now(),
      };
      return;
    }

    try {
      cameraValidation = await validateCameraSource(deviceId);
    } catch {
      cameraValidation = {
        id: deviceId,
        name: selectedCameraName,
        status: "unknown",
        statusMessage: "Camera validation could not complete.",
        probedAtUnixMs: Date.now(),
      };
    }
  }

  onMount(() => {
    const html = document.documentElement;
    const body = document.body;
    html.style.background = "transparent";
    html.style.overflow = "hidden";
    html.style.scrollbarGutter = "auto";
    (
      html.style as CSSStyleDeclaration & { scrollbarWidth?: string }
    ).scrollbarWidth = "none";
    body.style.background = "transparent";
    body.style.overflow = "hidden";
    body.style.margin = "0";

    const timer = window.setInterval(() => {
      if (isRecording) now = Date.now();
    }, 1000);

    const unlistenSource = listen<TargetSource>("source-selected", (event) => {
      selectedSource = event.payload;
      // Persist for next launch.
      setLastSource({
        kind:
          event.payload.type === "monitor"
            ? "monitor"
            : event.payload.type === "window"
              ? "window"
              : "region",
        id: event.payload.id,
        label: event.payload.label,
        regionX: event.payload.region?.x ?? null,
        regionY: event.payload.region?.y ?? null,
        regionWidth: event.payload.region?.width ?? null,
        regionHeight: event.payload.region?.height ?? null,
      }).catch(() => {});
    });

    // Listen for device selection from picker windows
    const unlistenDevice = listen<{
      type: string;
      id: string | null;
      name: string;
    }>("device-selected", (event) => {
      const { type, id, name } = event.payload;
      if (type === "mic") {
        if (id) {
          micOn = true;
          selectedMicId = id;
          selectedMicName = name;
        } else {
          micOn = false;
        }
      } else if (type === "camera") {
        if (id) {
          cameraOn = true;
          selectedCameraId = id;
          selectedCameraName = name;
          void refreshCameraValidation(id);
          openCameraPreview(id);
        } else {
          cameraOn = false;
          cameraValidation = null;
          closeCameraPreview();
        }
      }
    });

    // Prefer the last-used source from persisted config; fall back to the
    // primary display if no last source is recorded.
    getLastSource()
      .then((last) => {
        if (last) {
          selectedSource = {
            type:
              last.kind === "window"
                ? "window"
                : last.kind === "region"
                  ? "region"
                  : "monitor",
            id: last.id,
            label: last.label,
            region:
              last.kind === "region" &&
              last.regionWidth != null &&
              last.regionHeight != null
                ? {
                    x: last.regionX ?? 0,
                    y: last.regionY ?? 0,
                    width: last.regionWidth,
                    height: last.regionHeight,
                  }
                : undefined,
          };
          return;
        }
        return getDisplays().then((displays) => {
          if (displays.length > 0 && !selectedSource) {
            const d = displays[0];
            selectedSource = {
              type: "monitor",
              id: d.id,
              label: d.isPrimary ? "Primary Display" : `Display ${d.id}`,
            };
          }
        });
      })
      .catch(() => {});

    getAudioDevices()
      .then((devices) => {
        const def = devices.find((d) => d.isDefault);
        if (def) {
          selectedMicId = def.id;
          selectedMicName = def.name;
        }
      })
      .catch(() => {});

    enumerateCameras()
      .then((cams) => {
        // Already sorted with non-virtual hardware first, so [0] prefers a
        // real webcam over Phone Link / OBS Virtual / etc.
        if (cams.length > 0) {
          selectedCameraId = cams[0].deviceId;
          selectedCameraName = cams[0].label;
          void refreshCameraValidation(cams[0].deviceId);
        }
      })
      .catch(() => {});

    return () => {
      window.clearInterval(timer);
      unlistenSource.then((fn) => fn());
      unlistenDevice.then((fn) => fn());
    };
  });

  function openSourceSelector() {
    if (isRecording) return;
    WebviewWindow.getByLabel("source-selector").then(async (existing) => {
      if (existing) {
        await existing.setFocus();
        return;
      }
      new WebviewWindow("source-selector", {
        url: "/select",
        title: "Select Source",
        width: 560,
        height: 440,
        center: true,
        decorations: false,
        transparent: true,
        shadow: false,
        resizable: false,
      });
    });
  }

  function openDevicePicker(type: "mic" | "camera") {
    if (isRecording) return;
    const label = `device-picker-${type}`;
    const selected = type === "mic" ? selectedMicId : selectedCameraId;
    WebviewWindow.getByLabel(label).then(async (existing) => {
      if (existing) {
        await existing.setFocus();
        return;
      }
      new WebviewWindow(label, {
        url: `/device-picker?type=${type}&selected=${selected ?? ""}`,
        title: `Select ${type === "mic" ? "Microphone" : "Camera"}`,
        width: 320,
        height: 340,
        center: true,
        decorations: false,
        transparent: true,
        shadow: false,
        resizable: false,
      });
    });
  }

  function openCameraPreview(deviceId: string) {
    WebviewWindow.getByLabel("camera-preview").then(async (existing) => {
      if (existing) {
        await existing.close();
      }
      new WebviewWindow("camera-preview", {
        url: `/camera-preview?deviceId=${encodeURIComponent(deviceId)}`,
        title: "Camera",
        width: 200,
        height: 200,
        decorations: false,
        transparent: true,
        shadow: false,
        alwaysOnTop: true,
        resizable: true,
        x: 40,
        y: 40,
      });
    });
  }

  function closeCameraPreview() {
    emit("camera-recording-stopped");
    emit("camera-stop");
    WebviewWindow.getByLabel("camera-preview").then(async (existing) => {
      if (existing) await existing.close();
    });
  }

  function closePanel() {
    closeCameraPreview();
    getCurrentWindow().close();
  }

  function toggleMic() {
    if (isRecording) return;
    if (micOn) {
      micOn = false;
    } else {
      openDevicePicker("mic");
    }
  }

  function toggleCamera() {
    if (isRecording) return;
    if (cameraOn) {
      cameraOn = false;
      closeCameraPreview();
    } else {
      openDevicePicker("camera");
    }
  }

  async function toggleRecording() {
    if (isRecording) {
      try {
        await stopRecording();
        isRecording = false;
        recordingStartTime = null;
        emit("camera-recording-stopped");
        emit("refresh-recordings");
      } catch (e) {
        alert(`Stop failed: ${e}\n\nMake sure ffmpeg is installed.`);
      }
    } else {
      if (!selectedSource) return;
      const options: RecordingOptions = {
        systemAudio: systemAudioOn,
        microphone: micOn,
        microphoneDeviceId: micOn ? selectedMicId : null,
        camera: cameraOn,
        // Rust feeds this directly to FFmpeg dshow as a DirectShow friendly
        // name — pass the label, not the browser deviceId hash.
        cameraDeviceId: cameraOn ? selectedCameraName : null,
      };
      try {
        const result = await startRecording(
          selectedSource.type,
          selectedSource.id,
          options,
          selectedSource.type === "region" && selectedSource.region
            ? selectedSource.region
            : null,
        );
        isRecording = true;
        now = Date.now();
        recordingStartTime = now;
        if (cameraOn) {
          emit("camera-recording-started", { startedAtUnixMs: now });
        }
        if (result.warnings.length > 0) {
          alert(result.warnings.join("\n"));
        }
      } catch (e) {
        alert(`Recording failed: ${e}`);
      }
    }
  }

  const elapsed = $derived(
    isRecording && recordingStartTime
      ? Math.floor((now - recordingStartTime) / 1000)
      : 0,
  );
  const timer = $derived(
    `${Math.floor(elapsed / 60)
      .toString()
      .padStart(2, "0")}:${(elapsed % 60).toString().padStart(2, "0")}`,
  );
</script>

<div
  class="group/panel relative mx-auto flex h-dvh overflow-hidden no-scrollbar w-full items-center gap-1 bg-background p-2 backdrop-blur-3xl border border-border-subtle rounded-lg shadow-craft-floating"
  data-tauri-drag-region
>
  <ButtonGroup>
    <!-- Record / Stop -->
    <Button
      onclick={toggleRecording}
      onmousedown={(e) => e.stopPropagation()}
      size={isRecording ? "sm" : "icon-sm"}
      variant={isRecording ? "destructive" : "default"}
      title={isRecording ? "Stop Recording" : "Start Recording"}
    >
      {#if isRecording}
        <Square
          size={12}
          strokeWidth={0}
          fill="currentColor"
          class="animate-pulse"
        />
      {:else}
        <Circle size={14} strokeWidth={0} fill="currentColor" />
      {/if}
      {#if isRecording}
        <span
          class="shrink-0 font-mono text-[13px] font-semibold tabular-nums text-foreground tracking-tight"
          data-tauri-drag-region
        >
          {timer}
        </span>
      {/if}
    </Button>

    <!-- Timer -->
  </ButtonGroup>

  <!-- Source -->
  <Button
    size="sm"
    disabled={isRecording}
    onclick={openSourceSelector}
    onmousedown={(e) => e.stopPropagation()}
    variant="ghost"
    class="group/source hover:scale-none"
  >
    {#if selectedSource?.type === "window"}
      <AppWindow
        size={12}
        strokeWidth={2}
        class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
      />
    {:else if selectedSource?.type === "region"}
      <Crop
        size={12}
        strokeWidth={2}
        class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
      />
    {:else}
      <Monitor
        size={12}
        strokeWidth={2}
        class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors"
      />
    {/if}
    <span
      class="max-w-35 truncate text-[12px] font-semibold tracking-tight text-foreground/60 group-hover/source:text-foreground/90 transition-colors"
    >
      {selectedSource?.label ?? "Select source"}
    </span>
    {#if !isRecording}
      <ChevronDown
        size={10}
        strokeWidth={3}
        class="shrink-0 text-foreground/20 transition-transform group-hover/source:translate-y-0.5"
      />
    {/if}
  </Button>

  <div class="shrink-0 px-1 ml-auto inline-flex items-center gap-1">
    <!-- Device toggles -->
    <ButtonGroup>
      <!-- System audio -->
      <Button
        size="icon-sm"
        variant={systemAudioOn ? "default_soft" : "outline"}
        disabled={isRecording}
        onclick={() => (systemAudioOn = !systemAudioOn)}
        onmousedown={(e) => e.stopPropagation()}
        title={systemAudioOn ? "System audio: on" : "System audio: off"}
      >
        {#if systemAudioOn}
          <Volume2 size={14} strokeWidth={2} />
        {:else}
          <VolumeOff size={14} strokeWidth={2} />
        {/if}
      </Button>

      <!-- Mic -->
      <Button
        variant={micOn ? "default_soft" : "outline"}
        size="icon-sm"
        disabled={isRecording}
        onclick={toggleMic}
        onmousedown={(e) => e.stopPropagation()}
        title={micOn ? `Mic: ${selectedMicName}` : "Microphone: off"}
      >
        {#if micOn}
          <Mic size={14} strokeWidth={2} />
        {:else}
          <MicOff size={14} strokeWidth={2} />
        {/if}
      </Button>

      <!-- Camera -->
      <Button
        disabled={isRecording}
        onclick={toggleCamera}
        onmousedown={(e) => e.stopPropagation()}
        variant={cameraOn
          ? cameraValidation?.status === "error"
            ? "destructive_soft"
            : "default_soft"
          : "outline"}
        size="icon-sm"
        title={cameraOn
          ? `Camera: ${selectedCameraName}${cameraValidation?.statusMessage ? ` — ${cameraValidation.statusMessage}` : ""}`
          : "Camera: off"}
      >
        {#if cameraOn}
          <Camera size={14} strokeWidth={2} />
        {:else}
          <CameraOff size={14} strokeWidth={2} />
        {/if}
      </Button>
    </ButtonGroup>
    <!-- Close -->
    <Button
      onclick={closePanel}
      onmousedown={(e) => e.stopPropagation()}
      title="Close"
      size="icon-sm"
      variant="ghost"
    >
      <X size={10} strokeWidth={2} class="shrink-0 text-destructive" />
    </Button>
  </div>
</div>
