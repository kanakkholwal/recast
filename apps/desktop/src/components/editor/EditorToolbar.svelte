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
    Undo2,
    Upload,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { Separator } from "@recast/ui/separator";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import ExportDialog from "./ExportDialog.svelte";
  import PresetPicker, { PRESETS, type Preset } from "./PresetPicker.svelte";

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
    // Map the preset's aspect string onto the store's OutputAspect. Anything
    // we don't recognise (e.g. "Source") falls back to the source-matched
    // canvas, the v1 default.
    const aspectMap: Record<
      string,
      import("$lib/stores/editor-store.svelte").OutputAspect
    > = {
      "16:9": "16:9",
      "9:16": "9:16",
      "1:1": "1:1",
      "1.91:1": "1.91:1",
    };
    store.outputAspect = aspectMap[preset.aspect] ?? "source";
    // Remember which preset was applied so the toolbar can surface it as
    // a chip — purely a UI affordance; the visual effects above are what
    // actually drive the renderer.
    store.lastAppliedPresetId = preset.id;
  }

  // Drop the active preset back to the source-matched canvas without
  // touching background / padding / blur — leaves the user's tweaks alone
  // but removes any letterbox bars. Visual mirror of `applyPreset` so undo
  // collapses the whole gesture into a single stack entry.
  function clearPreset() {
    if (
      store.outputAspect === "source" &&
      store.lastAppliedPresetId === null
    ) {
      return;
    }
    store.pushUndoState();
    store.outputAspect = "source";
    store.lastAppliedPresetId = null;
  }

  // Resolve the chip label from the persisted preset id. If the id no
  // longer exists in PRESETS (e.g. removed across versions) we fall back
  // to the raw aspect string so users still see something useful.
  const activePreset = $derived.by(() => {
    const id = store.lastAppliedPresetId;
    if (!id) return null;
    return PRESETS.find((p) => p.id === id) ?? null;
  });

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

    <!-- Active-preset chip. Only renders when a preset has been applied
         (per-project state). Click the body to re-open the picker; click
         the trailing × to drop back to source aspect. -->
    {#if activePreset || store.outputAspect !== "source"}
      <div
        class="flex h-6 items-center gap-1 rounded-md border border-primary/30 bg-primary/10 pl-1.5 pr-0.5 text-[11px] font-semibold text-primary"
      >
        <Tooltip.Root>
          <Tooltip.Trigger>
            <button
              type="button"
              onclick={() => (showPresetsPicker = true)}
              class="flex h-full items-center gap-1.5 cursor-pointer"
              aria-label="Change preset"
            >
              {#if activePreset}
                <span class="text-[10px] uppercase tracking-wider text-primary/70">
                  {activePreset.category}
                </span>
                <span class="text-foreground">{activePreset.label}</span>
              {/if}
              <span
                class="inline-flex h-4 items-center rounded border border-primary/40 bg-background/60 px-1 font-mono text-[9px] font-semibold text-primary"
              >
                {store.outputAspect === "source"
                  ? "Source"
                  : store.outputAspect}
              </span>
            </button>
          </Tooltip.Trigger>
          <Tooltip.Content>Change preset</Tooltip.Content>
        </Tooltip.Root>
        <Tooltip.Root>
          <Tooltip.Trigger>
            <button
              type="button"
              onclick={clearPreset}
              aria-label="Reset to source aspect"
              class="ml-0.5 flex size-5 cursor-pointer items-center justify-center rounded text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary"
            >
              <X size={10} strokeWidth={2.5} />
            </button>
          </Tooltip.Trigger>
          <Tooltip.Content>
            Reset to source aspect (drops letterbox bars; keeps your other
            tweaks)
          </Tooltip.Content>
        </Tooltip.Root>
      </div>
    {/if}

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
