<script lang="ts">
  import { Button } from "$components/ui/button";
  import * as DropdownMenu from "$components/ui/dropdown-menu";
  import * as Tooltip from "$components/ui/tooltip";

  import type {
    BackgroundType,
    EditorStore,
    ExportFormat,
    ExportQuality,
  } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
    ChevronDown,
    Crop,
    Download,
    LayoutGrid,
    LoaderCircle,
    Redo2,
    Trash2,
    Undo2,
  } from "@lucide/svelte";

  interface Props {
    store: EditorStore;
    filename?: string;
    onback?: () => void;
    onexport?: () => void;
  }

  let { store, filename = "Recording", onback, onexport }: Props = $props();

  let showFormatMenu = $state(false);
  let showQualityMenu = $state(false);
  let showPresetsMenu = $state(false);

  $effect(() => {
    if (showFormatMenu) {
      showPresetsMenu = false;
      showQualityMenu = false;
    }
  });

  $effect(() => {
    if (showQualityMenu) {
      showFormatMenu = false;
      showPresetsMenu = false;
    }
  });

  $effect(() => {
    if (showPresetsMenu) {
      showFormatMenu = false;
      showQualityMenu = false;
    }
  });

  const formats: { value: ExportFormat; label: string; desc: string }[] = [
    { value: "mp4", label: "MP4", desc: "Best quality, universal" },
    { value: "webm", label: "WebM", desc: "Web-optimized, smaller" },
    { value: "gif", label: "GIF", desc: "Animated, shareable" },
  ];

  const exportQualities: {
    value: ExportQuality;
    label: string;
    desc: string;
  }[] = [
    { value: "small", label: "Small", desc: "Up to 720p, lightweight share" },
    { value: "hd", label: "HD", desc: "Up to 1080p, balanced default" },
    { value: "4k", label: "4K", desc: "Up to 2160p, highest detail" },
    { value: "source", label: "Source", desc: "Keep original resolution" },
  ];

  const presets: {
    label: string;
    bg: BackgroundType;
    value?: string;
    padding: number;
    blur: number;
    layout?: "auto" | "crop";
  }[] = [
    {
      label: "Studio",
      bg: "gradient",
      value: "linear-gradient(135deg, #0f172a 0%, #1d4ed8 100%)",
      padding: 36,
      blur: 18,
      layout: "auto",
    },
    {
      label: "Focus",
      bg: "color",
      value: "#0b1120",
      padding: 24,
      blur: 0,
      layout: "auto",
    },
    {
      label: "Spotlight",
      bg: "wallpaper",
      value: "/wallpapers/wallpaper7.png",
      padding: 56,
      blur: 36,
      layout: "auto",
    },
    {
      label: "Edge to Edge",
      bg: "color",
      value: "#020617",
      padding: 0,
      blur: 0,
      layout: "crop",
    },
  ];

  function applyPreset(preset: (typeof presets)[0]) {
    store.pushUndoState();
    store.setBackground({
      type: preset.bg,
      value: preset.value ?? store.backgroundValue,
    });
    store.padding = preset.padding;
    store.backgroundBlur = preset.blur;
    if (preset.layout) {
      store.layoutMode = preset.layout;
    }
    showPresetsMenu = false;
  }
</script>

<div
  class="flex h-12 shrink-0 items-center justify-between border-b border-border bg-card/50 backdrop-blur-sm px-3"
>
  <!-- Left: Back + Filename -->
  <div class="flex items-center gap-2 min-w-0">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <button
          onclick={() => onback?.()}
          class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90"
          aria-label="Back to recordings"
        >
          <ArrowLeft size={18} />
        </button>
      </Tooltip.Trigger>
      <Tooltip.Content>Back to recordings</Tooltip.Content>
    </Tooltip.Root>

    <Tooltip.Root>
      <Tooltip.Trigger>
        <button
          class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground/50 transition-all duration-150 hover:bg-destructive/10 hover:text-destructive active:scale-90"
          aria-label="Delete recording"
        >
          <Trash2 size={15} />
        </button>
      </Tooltip.Trigger>
      <Tooltip.Content>Delete recording</Tooltip.Content>
    </Tooltip.Root>

    <div class="mx-1 h-5 w-px bg-border/50"></div>

    <span
      class="truncate text-sm font-medium text-foreground max-w-50"
      title={filename}
    >
      {filename}
    </span>
  </div>

  <!-- Center: Layout + Presets -->
  <div class="flex items-center gap-3">
    <!-- Layout Mode Toggle -->
    <div
      class="flex items-center gap-1 rounded-lg bg-muted/50 p-0.5 border border-border/50"
    >
      <button
        onclick={() => (store.layoutMode = "auto")}
        class="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-all duration-200
					{store.layoutMode === 'auto'
          ? 'bg-background text-foreground shadow-sm'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <LayoutGrid size={13} />
        Auto
      </button>
      <button
        onclick={() => (store.layoutMode = "crop")}
        class="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-all duration-200
					{store.layoutMode === 'crop'
          ? 'bg-background text-foreground shadow-sm'
          : 'text-muted-foreground hover:text-foreground'}"
      >
        <Crop size={13} />
        Crop
      </button>
    </div>
    <DropdownMenu.Root
      bind:open={showPresetsMenu}
    >
      <DropdownMenu.Trigger
        class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        <span class="text-sm">✦</span>
        Presets
        <ChevronDown
          size={12}
          class="transition-transform duration-200 {showPresetsMenu
            ? 'rotate-180'
            : ''}"
        />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content id="presets" preventScroll={false}>
          {#each presets as preset}
            <DropdownMenu.Item
              onclick={() => applyPreset(preset)}
              class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-muted"
            >
              {preset.label}
            </DropdownMenu.Item>
          {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  
    <div class="flex items-center gap-0.5">
      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            onclick={() => store.undo()}
            disabled={!store.canUndo}
            class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90 disabled:opacity-30 disabled:pointer-events-none"
            aria-label="Undo"
          >
            <Undo2 size={16} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>Undo (Ctrl+Z)</Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            onclick={() => store.redo()}
            disabled={!store.canRedo}
            class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90 disabled:opacity-30 disabled:pointer-events-none"
            aria-label="Redo"
          >
            <Redo2 size={16} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>Redo (Ctrl+Shift+Z)</Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>

  <!-- Right: Format + Export -->
  <div class="flex items-center gap-2">
    <!-- Format selector -->
    <DropdownMenu.Root bind:open={showFormatMenu}>
      <DropdownMenu.Trigger
        class="flex items-center gap-1 rounded-lg border border-border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        {store.exportFormat.toUpperCase()}
        <ChevronDown
          size={11}
          class="transition-transform duration-200 {showFormatMenu
            ? 'rotate-180'
            : ''}"
        />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        align="end"
        class="w-52"
        preventScroll={false}
      >
        {#each formats as fmt}
          <DropdownMenu.Item
            onclick={() => {
              store.exportFormat = fmt.value;
              showFormatMenu = false;
            }}
            class="flex w-full flex-col items-start gap-0 rounded-lg px-3 py-2 transition-colors hover:bg-muted
							{store.exportFormat === fmt.value ? 'bg-muted/50' : ''}"
          >
            <span class="text-xs font-semibold text-foreground">{fmt.label}</span>
            <span class="text-[10px] text-muted-foreground">{fmt.desc}</span>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <DropdownMenu.Root bind:open={showQualityMenu}>
      <DropdownMenu.Trigger
        class="flex items-center gap-1 rounded-lg border border-border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        {store.exportQuality.toUpperCase()}
        <ChevronDown
          size={11}
          class="transition-transform duration-200 {showQualityMenu
            ? 'rotate-180'
            : ''}"
        />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        align="end"
        class="w-56"
        preventScroll={false}
      >
        {#each exportQualities as quality}
          <DropdownMenu.Item
            onclick={() => {
              store.exportQuality = quality.value;
              showQualityMenu = false;
            }}
            class="flex w-full flex-col items-start gap-0 rounded-lg px-3 py-2 transition-colors hover:bg-muted
              {store.exportQuality === quality.value ? 'bg-muted/50' : ''}"
          >
            <span class="text-xs font-semibold text-foreground">{quality.label}</span>
            <span class="text-[10px] text-muted-foreground">{quality.desc}</span>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <!-- Export Button -->
    <Button
      onclick={() => onexport?.()}
      disabled={store.isExporting}
      class="relative overflow-hidden bg-linear-to-r from-blue-600 to-blue-500 text-white shadow-md hover:from-blue-500 hover:to-blue-400 active:scale-95 transition-all duration-200 h-8 px-4 text-xs font-semibold rounded-lg"
    >
      {#if store.isExporting}
        <LoaderCircle size={16} class="animate-spin" />
        Exporting...
      {:else}
        <Download size={14} />
        Export
      {/if}
    </Button>
  </div>
</div>

