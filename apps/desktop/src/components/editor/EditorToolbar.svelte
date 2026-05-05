<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
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
  import { Kbd } from "@recast/ui/kbd";
  import { Separator } from "@recast/ui/separator";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import ExportDialog from "./ExportDialog.svelte";
  import PresetPicker, { type Preset } from "./PresetPicker.svelte";

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
  let showPresetsPicker = $state(false);
  let exportDialogOpen = $state(false);

  const layoutModes: { value: "auto" | "crop"; label: string; icon: typeof LayoutGrid }[] = [
    { value: "auto", label: "Auto", icon: LayoutGrid },
    { value: "crop", label: "Crop", icon: Crop },
  ];

  function applyPreset(preset: Preset) {
    store.pushUndoState();
    store.setBackground({
      type: preset.bg,
      value: preset.value ?? store.backgroundValue,
    });
    store.padding = preset.padding;
    store.backgroundBlur = preset.blur;
    if (preset.layout) store.layoutMode = preset.layout;
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
  <!-- Left: back + filename -->
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
    class="truncate text-[11px] font-semibold tracking-tight text-foreground max-w-52"
    title={filename}
    data-tauri-drag-region
  >
    {filename}
  </span>
  {#if store.isDirty}
    <span
      class="size-1.5 rounded-full bg-primary"
      aria-hidden="true"
      title="Unsaved changes"
    ></span>
  {/if}

  <!-- Center: layout segmented + presets + undo/redo -->
  <div class="mx-auto flex items-center gap-1.5" data-tauri-drag-region>
    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
      role="radiogroup"
      aria-label="Layout mode"
    >
      {#each layoutModes as m (m.value)}
        {@const Icon = m.icon}
        {@const active = store.layoutMode === m.value}
        <button
          type="button"
          role="radio"
          aria-checked={active}
          onclick={() => (store.layoutMode = m.value)}
          class={cn(
            "flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold transition-all duration-150",
            active
              ? "bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon class="size-3" />
          <span>{m.label}</span>
        </button>
      {/each}
    </div>

    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant="ghost"
          size="xs"
          class="gap-1.5 text-[11px] text-muted-foreground"
          onclick={() => (showPresetsPicker = true)}
        >
          <Sparkles size={12} />
          Presets
          <Kbd class="ml-1">⌘P</Kbd>
        </Button>
      </Tooltip.Trigger>
      <Tooltip.Content>Browse social & studio presets</Tooltip.Content>
    </Tooltip.Root>

    <Separator orientation="vertical" class="mx-0.5 h-3.5" />

    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
    >
      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            type="button"
            onclick={() => store.undo()}
            disabled={!store.canUndo}
            aria-label="Undo"
            class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
          >
            <Undo2 size={12} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            Undo <Kbd>Ctrl+Z</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            type="button"
            onclick={() => store.redo()}
            disabled={!store.canRedo}
            aria-label="Redo"
            class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
          >
            <Redo2 size={12} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            Redo <Kbd>Ctrl+Shift+Z</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>

  <!-- Right: save + export -->
  <div class="ml-auto flex items-center gap-1">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button
          variant={store.isDirty ? "secondary" : "ghost"}
          size="xs"
          class="gap-1.5 text-[11px]"
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
        {#if store.isDirty}
          <span class="inline-flex items-center gap-1.5">
            Save project <Kbd>Ctrl+S</Kbd>
          </span>
        {:else}
          No unsaved changes
        {/if}
      </Tooltip.Content>
    </Tooltip.Root>

    <Button
      onclick={openExport}
      disabled={store.isExporting}
      size="xs"
      class="gap-1.5 text-[11px]"
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

<PresetPicker
  open={showPresetsPicker}
  onOpenChange={(v) => (showPresetsPicker = v)}
  onapply={applyPreset}
/>

<svelte:window
  onkeydown={(e) => {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "p") {
      e.preventDefault();
      showPresetsPicker = true;
    }
  }}
/>
