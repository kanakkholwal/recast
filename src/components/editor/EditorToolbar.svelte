<script lang="ts">
  import { Button } from "$components/ui/button";
  import * as DropdownMenu from "$components/ui/dropdown-menu";
  import * as Tooltip from "$components/ui/tooltip";

  import type {
    BackgroundType,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
    ChevronDown,
    Download,
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

<!-- Titlebar content — rendered inside CustomTitlebar -->
<div class="flex items-center justify-between w-full h-full px-2" data-tauri-drag-region>
  <!-- Left: Back + Delete + Filename -->
  <div class="flex items-center gap-1.5 min-w-0">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <button
          onclick={() => onback?.()}
          class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90"
          aria-label="Back to recordings"
        >
          <ArrowLeft size={16} />
        </button>
      </Tooltip.Trigger>
      <Tooltip.Content>Back to recordings</Tooltip.Content>
    </Tooltip.Root>

    <Tooltip.Root>
      <Tooltip.Trigger>
        <button
          class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground/40 transition-all duration-150 hover:bg-destructive/10 hover:text-destructive active:scale-90"
          aria-label="Delete recording"
        >
          <Trash2 size={14} />
        </button>
      </Tooltip.Trigger>
      <Tooltip.Content>Delete recording</Tooltip.Content>
    </Tooltip.Root>

    <div class="mx-0.5 h-4 w-px bg-border/40"></div>

    <span
      class="truncate text-xs font-medium text-muted-foreground max-w-44"
      title={filename}
    >
      {filename}
    </span>
  </div>

  <!-- Center: Presets + Undo/Redo -->
  <div class="flex items-center gap-2" data-tauri-drag-region>
    <DropdownMenu.Root bind:open={showPresetsMenu}>
      <DropdownMenu.Trigger
        class="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        <Sparkles size={13} />
        Presets
        <ChevronDown
          size={11}
          class="transition-transform duration-200 {showPresetsMenu ? 'rotate-180' : ''}"
        />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content preventScroll={false}>
        {#each presets as preset}
          <DropdownMenu.Item
            onclick={() => applyPreset(preset)}
            class="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted"
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
            class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90 disabled:opacity-30 disabled:pointer-events-none"
            aria-label="Undo"
          >
            <Undo2 size={14} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>Undo (Ctrl+Z)</Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            onclick={() => store.redo()}
            disabled={!store.canRedo}
            class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground active:scale-90 disabled:opacity-30 disabled:pointer-events-none"
            aria-label="Redo"
          >
            <Redo2 size={14} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>Redo (Ctrl+Shift+Z)</Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>

  <!-- Right: Export -->
  <div class="flex items-center gap-2 mr-1">
    <Button
      onclick={() => onexport?.()}
      disabled={store.isExporting}
      class="relative overflow-hidden bg-linear-to-r from-blue-600 to-blue-500 text-white shadow-sm hover:from-blue-500 hover:to-blue-400 active:scale-95 transition-all duration-200 h-7 px-3.5 text-xs font-semibold rounded-md"
    >
      {#if store.isExporting}
        <LoaderCircle size={14} class="animate-spin" />
        Exporting...
      {:else}
        <Download size={13} />
        Export
      {/if}
    </Button>
  </div>
</div>
