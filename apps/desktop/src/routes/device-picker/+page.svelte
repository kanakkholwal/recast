<script lang="ts">
  import { enumerateCameras } from "$lib/camera/browser-devices";
  import {
    getAudioDevices,
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
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
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
      if (isMic) {
        devices = await getAudioDevices();
      } else {
        // Source cameras from the WebView's MediaDevices so the deviceId we
        // pass downstream is one getUserMedia({deviceId:{exact}}) will accept.
        // Sorted with non-virtual hardware first; the chosen "default" below
        // therefore prefers a real webcam over Phone Link / OBS Virtual / etc.
        const cams = await enumerateCameras();
        devices = cams.map<CameraDeviceInfo>((c) => ({
          id: c.deviceId,
          name: c.label,
          status: c.isVirtual ? "warning" : "ready",
          statusMessage: c.isVirtual ? "Virtual camera" : null,
        }));
      }
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
    emit("device-selected", {
      type: deviceType,
      id,
      name: devices.find((d) => d.id === id)?.name ?? "",
    });
    getCurrentWindow().close();
  }

  function turnOff() {
    emit("device-selected", { type: deviceType, id: null, name: "" });
    getCurrentWindow().close();
  }

  function closeWindow() {
    getCurrentWindow().close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeWindow();
      return;
    }
    if (isLoading || devices.length === 0) return;
    const idx = devices.findIndex((d) => d.id === currentSelectedId);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = devices[(idx + 1 + devices.length) % devices.length];
      currentSelectedId = next.id;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const prev =
        devices[(idx - 1 + devices.length) % devices.length] ?? devices[0];
      currentSelectedId = prev.id;
    } else if (e.key === "Enter" && currentSelectedId) {
      e.preventDefault();
      selectDevice(currentSelectedId);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class={cn(
    "group/root flex h-screen w-full flex-col overflow-hidden select-none rounded-2xl border border-border-subtle bg-card backdrop-blur-3xl",
    isLoading && "cursor-wait",
  )}
  aria-busy={isLoading}
  data-tauri-drag-region
>
  <!-- Header -->
  <header
    class="flex items-center justify-between border-b border-border-subtle px-4 h-10 shrink-0"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2">
      {#if isMic}
        <Mic size={11} class="text-muted-foreground" />
      {:else}
        <Camera size={11} class="text-muted-foreground" />
      {/if}
      <span
        class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
      >
        Select {title}
      </span>
    </div>
    <Button
      onclick={closeWindow}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      size="icon-sm"
      variant="ghost"
      class="opacity-0 group-hover/root:opacity-100 transition-opacity"
      title="Close (Esc)"
    >
      <X size={11} strokeWidth={2.5} />
    </Button>
  </header>

  <!-- Device list -->
  <div class="flex-1 overflow-y-auto px-2 py-2 scrollbar-transparent">
    {#if isLoading}
      <div
        class="flex flex-col gap-0.5"
        role="status"
        aria-live="polite"
        aria-label="Scanning {title.toLowerCase()}s"
      >
        <div class="mb-1 flex items-center gap-2 px-2 py-1.5">
          <RefreshCw
            size={12}
            class="animate-spin text-muted-foreground"
            strokeWidth={2}
          />
          <span class="text-[11px] font-medium text-muted-foreground">
            Scanning {title.toLowerCase()}s…
          </span>
        </div>
        {#each Array.from({ length: 3 }) as _, i (i)}
          <div
            class="flex animate-pulse items-center gap-2 rounded-md px-2 py-1.5"
            style="animation-delay: {i * 120}ms"
          >
            <div class="size-6 shrink-0 rounded-sm bg-muted/70"></div>
            <div class="flex-1 space-y-1">
              <div
                class="h-2 rounded-sm bg-muted/70"
                style="width: {[78, 62, 70][i]}%"
              ></div>
              {#if i === 0}
                <div class="h-1.5 w-1/3 rounded-sm bg-muted/50"></div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {:else if devices.length === 0}
      <div
        class="flex flex-col items-center justify-center h-40 gap-2 rounded-md border border-dashed border-border bg-card/40"
      >
        {#if isMic}
          <MicOff size={18} class="text-muted-foreground" />
        {:else}
          <CameraOff size={18} class="text-muted-foreground" />
        {/if}
        <p class="text-[11px] font-medium text-foreground">
          No {title.toLowerCase()}s found
        </p>
      </div>
    {:else}
      <div class="flex flex-col gap-0.5">
        {#each devices as device (device.id)}
          {@const active = currentSelectedId === device.id}
          <button
            type="button"
            onclick={() => selectDevice(device.id)}
            onmousedown={(e) => e.stopPropagation()}
            class={cn(
              "group flex items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
              "focus:outline-none focus:ring-1 focus:ring-ring",
              active
                ? "bg-primary/10 text-foreground"
                : "text-foreground/80 hover:bg-muted/60",
            )}
          >
            <div
              class={cn(
                "size-6 shrink-0 rounded-sm flex items-center justify-center",
                active
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground",
              )}
            >
              {#if isMic}
                <Mic size={12} strokeWidth={2} />
              {:else}
                <Camera size={12} strokeWidth={2} />
              {/if}
            </div>

            <div class="flex-1 min-w-0">
              <div
                class="truncate text-[11px] font-medium leading-tight text-foreground"
              >
                {device.name}
              </div>
              {#if isMic && "isDefault" in device && device.isDefault}
                <div
                  class="text-[10px] font-medium text-muted-foreground leading-tight mt-0.5"
                >
                  System default
                </div>
              {/if}
            </div>

            {#if active}
              <Check size={12} strokeWidth={3} class="text-primary shrink-0" />
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer -->
  <footer
    data-tauri-drag-region
    class="flex items-center justify-between border-t border-border-subtle bg-card/50 px-3 h-11 shrink-0"
  >
    <Button
      onclick={fetchDevices}
      disabled={isLoading}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant="ghost"
      size="xs"
      class="gap-1.5"
    >
      <RefreshCw size={11} class={isLoading ? "animate-spin" : ""} />
      Rescan
    </Button>
    <Button
      onclick={turnOff}
      disabled={isLoading}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant="destructive_soft"
      size="xs"
      class="gap-1.5"
    >
      {#if isMic}
        <MicOff size={11} />
      {:else}
        <CameraOff size={11} />
      {/if}
      Turn off
    </Button>
  </footer>
</div>

<style>
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
