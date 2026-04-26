<script lang="ts">
  import type { BackgroundType, EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
    ChevronDown,
    Crop,
    LayoutGrid,
    LoaderCircle,
    Redo2,
    Save,
    Sparkles,
    Trash2,
    Undo2,
    Upload,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Separator } from "@recast/ui/separator";
  import * as Tooltip from "@recast/ui/tooltip";
  import ExportDialog from "./ExportDialog.svelte";

  interface Props {
    store: EditorStore;
    filename?: string;
    onback?: () => void;
    onexport?: () => void;
    onsave?: () => void | Promise<void>;
    isSaving?: boolean;
  }

  let {
    store,
    filename = "Recording",
    onback,
    onexport,
    onsave,
    isSaving = false,
  }: Props = $props();
  let showPresetsMenu = $state(false);
  let exportDialogOpen = $state(false);

  const presets: {
    label: string;
    bg: BackgroundType;
    value?: string;
    padding: number;
    blur: number;
    layout?: "auto" | "crop";
  }[] = [
    { label: "Studio", bg: "gradient", value: "linear-gradient(135deg, #0f172a 0%, #1d4ed8 100%)", padding: 3, blur: 18, layout: "auto" },
    { label: "Focus", bg: "color", value: "#0b1120", padding: 2, blur: 0, layout: "auto" },
    { label: "Spotlight", bg: "wallpaper", value: "asset:wallpaper7", padding: 5, blur: 36, layout: "auto" },
    { label: "Edge to Edge", bg: "color", value: "#020617", padding: 0, blur: 0, layout: "crop" },
  ];

  function applyPreset(preset: (typeof presets)[0]) {
    store.pushUndoState();
    store.setBackground({ type: preset.bg, value: preset.value ?? store.backgroundValue });
    store.padding = preset.padding;
    store.backgroundBlur = preset.blur;
    if (preset.layout) store.layoutMode = preset.layout;
    showPresetsMenu = false;
  }

  function openExport() {
    if (store.isExporting) return;
    exportDialogOpen = true;
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
          <ArrowLeft size={12} />
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
          <Trash2 size={12} />
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
        size="xs"
        class="gap-1 text-[11px]"
        onclick={() => (store.layoutMode = "auto")}
      >
        <LayoutGrid size={12} />
        Auto
      </Button>
      <Button
        variant={store.layoutMode === "crop" ? "secondary" : "ghost"}
        size="xs"
        class="gap-1 text-[11px]"
        onclick={() => (store.layoutMode = "crop")}
      >
        <Crop size={12} />
        Crop
      </Button>
    </div>

    <DropdownMenu.Root bind:open={showPresetsMenu}>
      <DropdownMenu.Trigger>
        <Button variant="ghost" size="xs" class="gap-1 text-[11px] text-muted-foreground">
          <Sparkles size={12} />
          Presets
          <ChevronDown size={11} class="transition-transform {showPresetsMenu ? 'rotate-180' : ''}" />
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
          <Undo2 size={12} />
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
          <Redo2 size={12} />
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Redo (Ctrl+Shift+Z)</Tooltip.Content>
    </Tooltip.Root>
  </div>

  <!-- Right: save + export -->
  <div class="ml-auto flex items-center gap-1">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant={store.isDirty ? "secondary" : "ghost"}
          size="xs"
          class="gap-1 text-[11px]"
          onclick={() => onsave?.()}
          disabled={isSaving || (!store.isDirty && !isSaving)}
          aria-label="Save project"
        >
          {#if isSaving}
            <LoaderCircle size={12} class="animate-spin" />
            Saving…
          {:else}
            <Save size={12} />
            {store.isDirty ? "Save" : "Saved"}
          {/if}
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>
        {store.isDirty ? "Save project (Ctrl+S)" : "No unsaved changes"}
      </Tooltip.Content>
    </Tooltip.Root>

    <Button
      onclick={openExport}
      disabled={store.isExporting}
      size="xs"
      class="gap-1 text-[11px]"
    >
      {#if store.isExporting}
        <LoaderCircle size={12} class="animate-spin" />
        Exporting…
      {:else}
        <Upload size={12} />
        Export
      {/if}
    </Button>
  </div>
</div>

<ExportDialog
  {store}
  bind:open={exportDialogOpen}
  onOpenChange={(v) => (exportDialogOpen = v)}
  onConfirm={() => onexport?.()}
/>
