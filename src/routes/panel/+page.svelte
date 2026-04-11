<script lang="ts">
  import Button from "$components/ui/button/button.svelte";
  import {
    getAudioDevices,
    getCameraDevices,
    getDisplays,
    startRecording,
    stopRecording,
    type RecordingOptions,
  } from "$lib/ipc";
  import {
    AppWindow,
    Camera,
    CameraOff,
    ChevronDown,
    Circle,
    Mic,
    MicOff,
    Monitor,
    Square,
    Volume2,
    VolumeOff,
    X,
  } from "@lucide/svelte";
  import { emit, listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  type TargetSource = {
    type: "monitor" | "window";
    id: number;
    label: string;
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

  onMount(() => {
    const timer = window.setInterval(() => {
      if (isRecording) now = Date.now();
    }, 1000);

    const unlistenSource = listen<TargetSource>("source-selected", (event) => {
      selectedSource = event.payload;
    });

    // Listen for device selection from picker windows
    const unlistenDevice = listen<{ type: string; id: string | null; name: string }>(
      "device-selected",
      (event) => {
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
            openCameraPreview(id);
          } else {
            cameraOn = false;
            closeCameraPreview();
          }
        }
      },
    );

    // Load defaults non-blocking
    getDisplays()
      .then((displays) => {
        if (displays.length > 0 && !selectedSource) {
          const d = displays[0];
          selectedSource = {
            type: "monitor",
            id: d.id,
            label: d.isPrimary ? "Primary Display" : `Display ${d.id}`,
          };
        }
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

    getCameraDevices()
      .then((devices) => {
        if (devices.length > 0) {
          selectedCameraId = devices[0].id;
          selectedCameraName = devices[0].name;
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
        alwaysOnTop: true,
        resizable: true,
        x: 40,
        y: 40,
      });
    });
  }

  function closeCameraPreview() {
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
        cameraDeviceId: cameraOn ? selectedCameraId : null,
      };
      try {
        await startRecording(selectedSource.type, selectedSource.id, options);
        isRecording = true;
        now = Date.now();
        recordingStartTime = now;
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
    `${Math.floor(elapsed / 60).toString().padStart(2, "0")}:${(elapsed % 60).toString().padStart(2, "0")}`,
  );
</script>

<div
  class="min-h-dvh h-full w-full flex items-center gap-1 px-1.5 backdrop-blur-2xl bg-card shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.06)] select-none"
  data-tauri-drag-region
>
  <!-- Record / Stop -->
  <Button
  size="icon"
    onclick={toggleRecording}
    onmousedown={(e) => e.stopPropagation()}
    variant={isRecording ? "destructive" : "ghost"}
    class="size-7 rounded-full shrink-0 transition-all duration-200 active:scale-90"
    title={isRecording ? "Stop Recording" : "Start Recording"}
  >
    {#if isRecording}
      <Square size={10} strokeWidth={0} fill="currentColor" />
    {:else}
      <Circle size={10} strokeWidth={0} fill="currentColor" class="text-red-500" />
    {/if}
  </Button>

  <div class="w-px h-3.5 bg-white/8 shrink-0"></div>

  <!-- Source -->
  <button
    disabled={isRecording}
    onclick={openSourceSelector}
    onmousedown={(e) => e.stopPropagation()}
    class="flex items-center gap-1 min-w-0 px-1.5 py-1 rounded-md transition-colors hover:bg-card/6 disabled:opacity-35 disabled:pointer-events-none"
  >
    {#if selectedSource?.type === "window"}
      <AppWindow size={11} strokeWidth={2} class="shrink-0 text-foreground/40" />
    {:else}
      <Monitor size={11} strokeWidth={2} class="shrink-0 text-foreground/40" />
    {/if}
    <span class="text-[10.5px] font-medium text-foreground/70 truncate max-w-24">
      {selectedSource?.label ?? "Select source"}
    </span>
    {#if !isRecording}
      <ChevronDown size={9} class="shrink-0 text-foreground/25" />
    {/if}
  </button>

  <div class="w-px h-3.5 bg-white/8 shrink-0"></div>

  <!-- Device toggles -->
  <div class="flex items-center gap-0.5 shrink-0">
    <!-- System audio (simple toggle) -->
    <Button
      disabled={isRecording}
      onclick={() => (systemAudioOn = !systemAudioOn)}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="icon-sm"
      class="size-6 rounded-md {systemAudioOn ? 'text-primary' : 'text-muted-foreground/40'}"
      title={systemAudioOn ? "System audio: on" : "System audio: off"}
    >
      {#if systemAudioOn}
        <Volume2 size={12} strokeWidth={2} />
      {:else}
        <VolumeOff size={12} strokeWidth={2} />
      {/if}
    </Button>

    <!-- Mic (click = open device picker window) -->
    <Button
      disabled={isRecording}
      onclick={toggleMic}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="icon-sm"
      class="size-6 rounded-md {micOn ? 'text-primary' : 'text-muted-foreground/40'}"
      title={micOn ? `Mic: ${selectedMicName}` : "Microphone: off — click to select"}
    >
      {#if micOn}
        <Mic size={12} strokeWidth={2} />
      {:else}
        <MicOff size={12} strokeWidth={2} />
      {/if}
    </Button>

    <!-- Camera (click = open device picker + camera preview) -->
    <Button
      disabled={isRecording}
      onclick={toggleCamera}
      onmousedown={(e) => e.stopPropagation()}
      variant="ghost"
      size="icon-sm"
      class="size-6 rounded-md {cameraOn ? 'text-primary' : 'text-muted-foreground/40'}"
      title={cameraOn ? `Camera: ${selectedCameraName}` : "Camera: off — click to select"}
    >
      {#if cameraOn}
        <Camera size={12} strokeWidth={2} />
      {:else}
        <CameraOff size={12} strokeWidth={2} />
      {/if}
    </Button>
  </div>

  <!-- Timer -->
  <span
    class="font-mono text-[10.5px] tabular-nums text-card-foreground/30 shrink-0 ml-auto"
    data-tauri-drag-region
  >
    {timer}
  </span>

  <!-- Close -->
  <Button
    onclick={closePanel}
    onmousedown={(e) => e.stopPropagation()}
    title="Close"
    variant="secondary"
    size="icon-sm"
    class="rounded-full"
  >
    <X size={10} class="shrink-0 size-3" />
  </Button>
</div>
