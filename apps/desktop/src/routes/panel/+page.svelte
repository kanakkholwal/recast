<script lang="ts">
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
  import { Button } from "@recast/ui/button";
  import { emit, listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { cn } from "@recast/ui/utils";

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
	class="group/panel relative mx-auto flex h-12 w-fit items-center gap-1 rounded-full bg-background/70 px-2 backdrop-blur-3xl border border-border-subtle shadow-craft-floating"
	data-tauri-drag-region
>
	<div class="flex items-center gap-2.5 px-1">
		<!-- Record / Stop -->
		<button
			onclick={toggleRecording}
			onmousedown={(e) => e.stopPropagation()}
			class={cn(
				"size-10 shrink-0 rounded-full flex items-center justify-center transition-all duration-300 hover:scale-105 active:scale-95 shadow-craft-sm",
				isRecording ? "bg-destructive text-white" : "bg-foreground/5 text-red-500 hover:bg-foreground/10"
			)}
			title={isRecording ? "Stop Recording" : "Start Recording"}
		>
			{#if isRecording}
				<Square size={12} strokeWidth={0} fill="currentColor" class="animate-pulse" />
			{:else}
				<Circle size={14} strokeWidth={0} fill="currentColor" />
			{/if}
		</button>

		<!-- Timer -->
		{#if isRecording}
			<span
				class="shrink-0 font-mono text-[13px] font-semibold tabular-nums text-foreground tracking-tight"
				data-tauri-drag-region
			>
				{timer}
			</span>
		{/if}
	</div>

	<div class="h-6 w-[1px] shrink-0 bg-foreground/[0.06] mx-2"></div>

	<!-- Source -->
	<button
		disabled={isRecording}
		onclick={openSourceSelector}
		onmousedown={(e) => e.stopPropagation()}
		class="group/source flex items-center gap-2 rounded-[12px] bg-foreground/[0.03] px-2.5 py-1.5 transition-all duration-300 hover:bg-foreground/[0.06] hover:scale-[1.02] active:scale-100 disabled:opacity-30 disabled:pointer-events-none"
	>
		{#if selectedSource?.type === "window"}
			<AppWindow size={13} strokeWidth={2} class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors" />
		{:else}
			<Monitor size={13} strokeWidth={2} class="shrink-0 text-foreground/30 group-hover/source:text-foreground/50 transition-colors" />
		{/if}
		<span class="max-w-[140px] truncate text-[12px] font-semibold tracking-tight text-foreground/60 group-hover/source:text-foreground/90 transition-colors">
			{selectedSource?.label ?? "Select source"}
		</span>
		{#if !isRecording}
			<ChevronDown size={10} strokeWidth={3} class="shrink-0 text-foreground/20 transition-transform group-hover/source:translate-y-0.5" />
		{/if}
	</button>

	<div class="h-6 w-[1px] shrink-0 bg-foreground/[0.06] mx-2"></div>

	<!-- Device toggles -->
	<div class="flex items-center gap-1 shrink-0 px-1">
		<!-- System audio -->
		<button
			disabled={isRecording}
			onclick={() => (systemAudioOn = !systemAudioOn)}
			onmousedown={(e) => e.stopPropagation()}
			class="size-9 rounded-[12px] flex items-center justify-center transition-all duration-300 {systemAudioOn ? 'bg-primary/[0.08] text-primary' : 'text-foreground/20 hover:text-foreground/40 hover:bg-foreground/[0.03]'}"
			title={systemAudioOn ? "System audio: on" : "System audio: off"}
		>
			{#if systemAudioOn}
				<Volume2 size={14} strokeWidth={2} />
			{:else}
				<VolumeOff size={14} strokeWidth={2} />
			{/if}
		</button>

		<!-- Mic -->
		<button
			disabled={isRecording}
			onclick={toggleMic}
			onmousedown={(e) => e.stopPropagation()}
			class="size-9 rounded-[12px] flex items-center justify-center transition-all duration-300 {micOn ? 'bg-primary/[0.08] text-primary' : 'text-foreground/20 hover:text-foreground/40 hover:bg-foreground/[0.03]'}"
			title={micOn ? `Mic: ${selectedMicName}` : "Microphone: off"}
		>
			{#if micOn}
				<Mic size={14} strokeWidth={2} />
			{:else}
				<MicOff size={14} strokeWidth={2} />
			{/if}
		</button>

		<!-- Camera -->
		<button
			disabled={isRecording}
			onclick={toggleCamera}
			onmousedown={(e) => e.stopPropagation()}
			class="size-9 rounded-[12px] flex items-center justify-center transition-all duration-300 {cameraOn ? 'bg-primary/[0.08] text-primary' : 'text-foreground/20 hover:text-foreground/40 hover:bg-foreground/[0.03]'}"
			title={cameraOn ? `Camera: ${selectedCameraName}` : "Camera: off"}
		>
			{#if cameraOn}
				<Camera size={14} strokeWidth={2} />
			{:else}
				<CameraOff size={14} strokeWidth={2} />
			{/if}
		</button>
	</div>

	<!-- Close -->
	<div class="pl-2 pr-1">
		<button
			onclick={closePanel}
			onmousedown={(e) => e.stopPropagation()}
			title="Close"
			class="size-7 rounded-full flex items-center justify-center transition-all duration-300 bg-foreground/[0.03] text-foreground/20 hover:bg-destructive/10 hover:text-destructive opacity-0 group-hover/panel:opacity-100"
		>
			<X size={10} strokeWidth={3} />
		</button>
	</div>
</div>
