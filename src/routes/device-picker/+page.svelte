<script lang="ts">
  import {
    getAudioDevices,
    getCameraDevices,
    type AudioDeviceInfo,
    type CameraDeviceInfo,
  } from "$lib/ipc";
  import {
    Camera,
    CameraOff,
    Check,
    Mic,
    MicOff,
    RefreshCw,
    X,
  } from "@lucide/svelte";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  // Determine device type from URL query: ?type=mic or ?type=camera
  const params = new URLSearchParams(window.location.search);
  const deviceType = params.get("type") === "camera" ? "camera" : "mic";
  const selectedId = params.get("selected") ?? null;

  let devices = $state<(AudioDeviceInfo | CameraDeviceInfo)[]>([]);
  let currentSelectedId = $state<string | null>(selectedId);
  let isLoading = $state(true);

  const isMic = deviceType === "mic";
  const title = isMic ? "Microphone" : "Camera";

  onMount(() => {
    fetchDevices();
  });

  async function fetchDevices() {
    isLoading = true;
    try {
      devices = isMic ? await getAudioDevices() : await getCameraDevices();
      if (!currentSelectedId && devices.length > 0) {
        const def = isMic
          ? (devices as AudioDeviceInfo[]).find((d) => d.isDefault)
          : devices[0];
        if (def) currentSelectedId = def.id;
      }
    } catch (e) {
      console.error(e);
    } finally {
      isLoading = false;
    }
  }

  function selectDevice(id: string) {
    currentSelectedId = id;
    emit("device-selected", { type: deviceType, id, name: devices.find((d) => d.id === id)?.name ?? "" });
    getCurrentWindow().close();
  }

  function turnOff() {
    emit("device-selected", { type: deviceType, id: null, name: "" });
    getCurrentWindow().close();
  }

  function closeWindow() {
    getCurrentWindow().close();
  }
</script>

<div class="flex h-screen w-full flex-col bg-background text-foreground font-sans overflow-hidden">
  <!-- Header -->
  <div class="flex items-center justify-between px-4 pt-3 pb-2 shrink-0">
    <span class="text-[13px] font-semibold tracking-tight text-card-foreground/90">
      Select {title}
    </span>
    <button
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      class="size-6 rounded-md flex items-center justify-center text-card-foreground/30 hover:text-card-foreground/70 hover:bg-card/8 transition-colors"
    >
      <X size={14} strokeWidth={2} />
    </button>
  </div>

  <!-- Device list -->
  <div class="flex-1 overflow-y-auto px-3 pb-3">
    {#if isLoading}
      <div class="flex items-center justify-center h-20">
        <RefreshCw size={16} class="animate-spin text-card-foreground/30" />
      </div>
    {:else if devices.length === 0}
      <div class="flex flex-col items-center justify-center h-20 gap-2">
        {#if isMic}
          <MicOff size={20} class="text-card-foreground/15" />
        {:else}
          <CameraOff size={20} class="text-card-foreground/15" />
        {/if}
        <p class="text-[11px] text-card-foreground/30">No {title.toLowerCase()} devices found</p>
      </div>
    {:else}
      <div class="space-y-1">
        {#each devices as device}
          <button
            onclick={() => selectDevice(device.id)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-all duration-150
              {currentSelectedId === device.id
              ? 'bg-card/8 text-card-foreground'
              : 'text-card-foreground/60 hover:bg-card/5 hover:text-card-foreground/80'}"
          >
            {#if isMic}
              <Mic size={14} class="shrink-0 text-emerald-400/70" />
            {:else}
              <Camera size={14} class="shrink-0 text-violet-400/70" />
            {/if}
            <div class="flex-1 min-w-0">
              <span class="text-[12px] font-medium truncate block">{device.name}</span>
              {#if isMic && "isDefault" in device && device.isDefault}
                <span class="text-[9px] text-card-foreground/30">Default device</span>
              {/if}
            </div>
            {#if currentSelectedId === device.id}
              <Check size={14} class="shrink-0 {isMic ? 'text-emerald-400' : 'text-violet-400'}" />
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer -->
  <div class="flex items-center justify-between px-4 py-3 border-t border-border/6 shrink-0">
    <button
      onclick={fetchDevices}
      disabled={isLoading}
      onmousedown={(e) => e.stopPropagation()}
      class="flex items-center gap-1.5 text-[11px] font-medium text-card-foreground/30 hover:text-card-foreground/60 transition-colors disabled:opacity-40"
    >
      <RefreshCw size={12} class={isLoading ? "animate-spin" : ""} />
      Refresh
    </button>
    <button
      onclick={turnOff}
      onmousedown={(e) => e.stopPropagation()}
      class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[11px] font-medium text-card-foreground/40 hover:text-card-foreground/70 hover:bg-card/5 transition-colors"
    >
      {#if isMic}
        <MicOff size={12} />
      {:else}
        <CameraOff size={12} />
      {/if}
      Turn off
    </button>
  </div>
</div>
