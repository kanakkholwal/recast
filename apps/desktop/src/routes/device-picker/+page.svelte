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
  import { cn } from "@recast/ui/utils";

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

<div class="flex h-screen w-full flex-col bg-background/50 backdrop-blur-3xl text-foreground font-sans overflow-hidden select-none">
  <!-- Header -->
  <div class="group/header flex items-center justify-between px-7 pt-7 pb-4 shrink-0" data-tauri-drag-region>
    <div class="space-y-1">
      <h1 class="text-xl font-semibold tracking-tight text-foreground">
        {title}
      </h1>
      <p class="text-[11px] font-medium text-foreground/40 uppercase tracking-[0.15em]">Select input device</p>
    </div>
    <button
      onclick={closeWindow}
      onmousedown={(e) => e.stopPropagation()}
      class="size-8 rounded-full flex items-center justify-center text-foreground/20 hover:text-foreground hover:bg-foreground/5 opacity-0 group-hover/header:opacity-100 transition-all duration-200"
    >
      <X size={14} strokeWidth={2.5} />
    </button>
  </div>

  <!-- Device list -->
  <div class="flex-1 overflow-y-auto px-7 pb-4 scrollbar-transparent">
    {#if isLoading}
      <div class="flex items-center justify-center h-40">
        <RefreshCw size={20} class="animate-spin text-primary/30" strokeWidth={1.5} />
      </div>
    {:else if devices.length === 0}
      <div class="flex flex-col items-center justify-center h-40 gap-4 bg-foreground/[0.02] rounded-3xl border border-border-subtle">
        {#if isMic}
          <MicOff size={24} class="text-foreground/10" strokeWidth={1.5} />
        {:else}
          <CameraOff size={24} class="text-foreground/10" strokeWidth={1.5} />
        {/if}
        <p class="text-[12px] font-medium text-foreground/30">No {title.toLowerCase()}s found</p>
      </div>
    {:else}
      <div class="space-y-3">
        {#each devices as device}
          <button
            onclick={() => selectDevice(device.id)}
            class={cn(
              "w-full flex items-center gap-4 px-4 py-4 rounded-2xl text-left transition-all duration-300 group relative overflow-hidden border",
              currentSelectedId === device.id
              ? "bg-primary/[0.03] border-primary/20 shadow-craft-sm"
              : "border-border-subtle hover:bg-card hover:border-border/40 hover:shadow-craft-md hover:scale-[1.01]"
            )}
          >
            <div class={cn(
              "size-10 rounded-xl flex items-center justify-center transition-all duration-300",
              currentSelectedId === device.id ? "bg-primary text-primary-foreground shadow-craft-sm" : "bg-foreground/[0.03] text-foreground/30 group-hover:text-foreground/50"
            )}>
              {#if isMic}
                <Mic size={16} strokeWidth={2} />
              {:else}
                <Camera size={16} strokeWidth={2} />
              {/if}
            </div>

            <div class="flex-1 min-w-0">
              <span class={cn(
                "text-[13px] font-semibold block truncate transition-colors",
                currentSelectedId === device.id ? "text-foreground" : "text-foreground/70"
              )}>
                {device.name}
              </span>
              {#if isMic && "isDefault" in device && device.isDefault}
                <span class="text-[10px] font-medium text-primary/50 uppercase tracking-wider mt-0.5 block">Suggested Device</span>
              {/if}
            </div>

            {#if currentSelectedId === device.id}
              <div class="size-6 bg-primary rounded-full flex items-center justify-center text-primary-foreground shadow-craft-sm animate-in zoom-in-50 duration-300">
                <Check size={12} strokeWidth={4} />
              </div>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer -->
  <div class="group/footer px-7 py-5 shrink-0 border-t border-border-subtle flex items-center justify-between">
    <button
      onclick={fetchDevices}
      disabled={isLoading}
      onmousedown={(e) => e.stopPropagation()}
      class="flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.15em] text-foreground/20 hover:text-foreground transition-all duration-300 disabled:opacity-40 opacity-0 group-hover/footer:opacity-100"
    >
      <RefreshCw size={12} strokeWidth={2.5} class={isLoading ? "animate-spin" : ""} />
      Rescan
    </button>
    <button
      onclick={turnOff}
      onmousedown={(e) => e.stopPropagation()}
      class="flex items-center gap-2 rounded-xl px-4 py-2 text-[11px] font-medium uppercase tracking-[0.15em] text-destructive/40 hover:text-destructive hover:bg-destructive/[0.05] transition-all duration-300"
    >
      {#if isMic}
        <MicOff size={12} strokeWidth={2.5} />
      {:else}
        <CameraOff size={12} strokeWidth={2.5} />
      {/if}
      Disable
    </button>
  </div>
</div>
