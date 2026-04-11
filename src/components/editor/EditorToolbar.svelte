<script lang="ts">
  import { Button } from "$components/ui/button";
  import * as DropdownMenu from "$components/ui/dropdown-menu";
  import { Separator } from "$components/ui/separator";
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
    Sparkles,
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
  let showPresetsMenu = $state(false);
  let showFormatMenu = $state(false);
  let showQualityMenu = $state(false);

  const presets: {
    label: string;
    bg: BackgroundType;
    value?: string;
    padding: number;
    blur: number;
    layout?: "auto" | "crop";
  }[] = [
    { label: "Studio", bg: "gradient", value: "linear-gradient(135deg, #0f172a 0%, #1d4ed8 100%)", padding: 36, blur: 18, layout: "auto" },
    { label: "Focus", bg: "color", value: "#0b1120", padding: 24, blur: 0, layout: "auto" },
    { label: "Spotlight", bg: "wallpaper", value: "/wallpapers/wallpaper7.png", padding: 56, blur: 36, layout: "auto" },
    { label: "Edge to Edge", bg: "color", value: "#020617", padding: 0, blur: 0, layout: "crop" },
  ];

  const formats: { value: ExportFormat; label: string; desc: string }[] = [
    { value: "mp4", label: "MP4", desc: "Best quality, universal" },
    { value: "webm", label: "WebM", desc: "Web-optimized, smaller" },
    { value: "gif", label: "GIF", desc: "Animated, shareable" },
  ];

  const qualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "Small", desc: "Up to 720p" },
    { value: "hd", label: "HD", desc: "Up to 1080p" },
    { value: "4k", label: "4K", desc: "Up to 2160p" },
    { value: "source", label: "Source", desc: "Original resolution" },
  ];

  function applyPreset(preset: (typeof presets)[0]) {
    store.pushUndoState();
    store.setBackground({ type: preset.bg, value: preset.value ?? store.backgroundValue });
    store.padding = preset.padding;
    store.backgroundBlur = preset.blur;
    if (preset.layout) store.layoutMode = preset.layout;
    showPresetsMenu = false;
  }
</script>

<div
  class="flex h-full w-full items-center gap-1.5 px-2 text-[11px]"
  data-tauri-drag-region
>
  <!-- Left: back + delete + filename -->
  <div class="flex items-center gap-0.5">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button variant="ghost" size="icon-sm" onclick={() => onback?.()} aria-label="Back">
          <ArrowLeft size={14} />
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Back to recordings</Tooltip.Content>
    </Tooltip.Root>

    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant="ghost"
          size="icon-sm"
          class="text-muted-foreground/60 hover:text-destructive"
          aria-label="Delete"
        >
          <Trash2 size={13} />
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Delete recording</Tooltip.Content>
    </Tooltip.Root>
  </div>

  <Separator orientation="vertical" class="mx-1 h-3.5" />

  <span
    class="truncate text-[11px] font-medium text-foreground max-w-52"
    title={filename}
    data-tauri-drag-region
  >
    {filename}
  </span>

  <!-- Center: layout + presets + undo/redo -->
  <div class="mx-auto flex items-center gap-1" data-tauri-drag-region>
    <div class="flex items-center gap-0.5 rounded-md border border-border bg-muted/40 p-0.5">
      <Button
        variant={store.layoutMode === "auto" ? "secondary" : "ghost"}
        size="sm"
        class="h-6 gap-1 px-2 text-[11px]"
        onclick={() => (store.layoutMode = "auto")}
      >
        <LayoutGrid size={11} />
        Auto
      </Button>
      <Button
        variant={store.layoutMode === "crop" ? "secondary" : "ghost"}
        size="sm"
        class="h-6 gap-1 px-2 text-[11px]"
        onclick={() => (store.layoutMode = "crop")}
      >
        <Crop size={11} />
        Crop
      </Button>
    </div>

    <DropdownMenu.Root bind:open={showPresetsMenu}>
      <DropdownMenu.Trigger>
        <Button variant="ghost" size="sm" class="h-6 gap-1 px-2 text-[11px] text-muted-foreground">
          <Sparkles size={11} />
          Presets
          <ChevronDown size={10} class="transition-transform {showPresetsMenu ? 'rotate-180' : ''}" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content preventScroll={false}>
        {#each presets as preset}
          <DropdownMenu.Item onclick={() => applyPreset(preset)}>{preset.label}</DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <Separator orientation="vertical" class="mx-0.5 h-3.5" />

    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant="ghost"
          size="icon-sm"
          onclick={() => store.undo()}
          disabled={!store.canUndo}
          aria-label="Undo"
        >
          <Undo2 size={13} />
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Undo (Ctrl+Z)</Tooltip.Content>
    </Tooltip.Root>

    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant="ghost"
          size="icon-sm"
          onclick={() => store.redo()}
          disabled={!store.canRedo}
          aria-label="Redo"
        >
          <Redo2 size={13} />
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Redo (Ctrl+Shift+Z)</Tooltip.Content>
    </Tooltip.Root>
  </div>

  <!-- Right: format + quality + export -->
  <div class="ml-auto flex items-center gap-1">
    <DropdownMenu.Root bind:open={showFormatMenu}>
      <DropdownMenu.Trigger>
        <Button
          variant="ghost"
          size="sm"
          class="h-6 gap-1 px-2 text-[11px] text-muted-foreground"
        >
          {store.exportFormat.toUpperCase()}
          <ChevronDown size={10} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-48" preventScroll={false}>
        {#each formats as fmt}
          <DropdownMenu.Item
            onclick={() => {
              store.exportFormat = fmt.value;
              showFormatMenu = false;
            }}
          >
            <div class="flex flex-col gap-0.5">
              <span class="text-[12px] font-medium">{fmt.label}</span>
              <span class="text-[10px] text-muted-foreground">{fmt.desc}</span>
            </div>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <DropdownMenu.Root bind:open={showQualityMenu}>
      <DropdownMenu.Trigger>
        <Button
          variant="ghost"
          size="sm"
          class="h-6 gap-1 px-2 text-[11px] text-muted-foreground"
        >
          {store.exportQuality.toUpperCase()}
          <ChevronDown size={10} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-48" preventScroll={false}>
        {#each qualities as q}
          <DropdownMenu.Item
            onclick={() => {
              store.exportQuality = q.value;
              showQualityMenu = false;
            }}
          >
            <div class="flex flex-col gap-0.5">
              <span class="text-[12px] font-medium">{q.label}</span>
              <span class="text-[10px] text-muted-foreground">{q.desc}</span>
            </div>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <Button
      onclick={() => onexport?.()}
      disabled={store.isExporting}
      size="sm"
      class="h-6 gap-1 px-2 text-[11px]"
    >
      {#if store.isExporting}
        <LoaderCircle size={12} class="animate-spin" />
        Exporting…
      {:else}
        <Download size={11} />
        Export
      {/if}
    </Button>
  </div>
</div>
