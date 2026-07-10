<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
    LoaderCircle,
    PanelBottom,
    PanelRight,
    RotateCcw,
    Save,
    Sparkles,
    Upload,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { Separator } from "@recast/ui/separator";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import ConfirmDialog from "../recast/ConfirmDialog.svelte";
  import PresetPicker, { PRESETS, type Preset } from "./PresetPicker.svelte";
  import { onMount } from "svelte";
  import { registerShortcutHandlers } from "$lib/shortcuts/registry.svelte";

  interface Props {
    store: EditorStore;
    filename?: string;
    onexport?: () => void;
    onsave?: () => void | Promise<void>;
    isSaving?: boolean;
    showSidebar?: boolean;
    showTimeline?: boolean;
    onToggleSidebar?: () => void;
    onToggleTimeline?: () => void;
  }

  let {
    store,
    filename = "Recording",
    onexport,
    onsave,
    isSaving = false,
    showSidebar = true,
    showTimeline = true,
    onToggleSidebar,
    onToggleTimeline,
  }: Props = $props();

  const toggleClass = (active: boolean) =>
    cn(
      "cursor-pointer flex size-6 items-center justify-center rounded-md transition-colors duration-150",
      active
        ? "text-foreground shadow-(--shadow-craft-inset)"
        : "text-muted-foreground hover:bg-card/60 hover:text-foreground",
    );
  let showPresetsPicker = $state(false);
  let showRevertConfirm = $state(false);

  // Mod+P via the central shortcut registry, which avoids a per-component window listener leaking under HMR.
  onMount(() =>
    registerShortcutHandlers({
      "editor.presets": () => {
        showPresetsPicker = true;
      },
    }),
  );

  function applyPreset(preset: Preset) {
    store.pushUndoState();
    store.setBackground({
      type: preset.bg,
      value: preset.value ?? store.backgroundValue,
    });
    store.padding = preset.padding;
    store.backgroundBlur = preset.blur;
    if (preset.layout) store.layoutMode = preset.layout;
    // Unrecognised aspects (e.g. "Source") fall back to the source-matched canvas.
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
    // UI-only: lets the toolbar surface the applied preset as a chip.
    store.lastAppliedPresetId = preset.id;
  }

  // Reset to source aspect (removes letterbox bars) without touching background/padding/blur.
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

  // null if the persisted id no longer exists in PRESETS (removed across versions).
  const activePreset = $derived.by(() => {
    const id = store.lastAppliedPresetId;
    if (!id) return null;
    return PRESETS.find((p) => p.id === id) ?? null;
  });

  function openExport() {
    if (store.isExporting) return;
    onexport?.();
  }
</script>

<div
  class="flex h-full w-full items-center gap-1.5 px-2 text-[11px]"
  
>
  <div class="flex items-center gap-0.5">
    <Tooltip.Root>
      <Tooltip.Trigger>
        <Button variant="ghost" size="icon-sm" href="/recasts" aria-label="Back">
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

  <div class="mx-auto flex items-center gap-1.5">
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
  </div>

  <div class="ml-auto flex items-center gap-1">
    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
    >
    {#if store.canRevert}
      <Tooltip.Root>
        <Tooltip.Trigger>
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5 text-[11px] text-muted-foreground hover:text-destructive"
            onclick={() => (showRevertConfirm = true)}
            disabled={isSaving}
            aria-label="Revert unsaved changes"
          >
            <RotateCcw size={12} />
            Revert
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          Discard unsaved changes and restore the last saved state
        </Tooltip.Content>
      </Tooltip.Root>
    {/if}

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
        <Separator orientation="vertical" class="mx-0.5 h-3.5" />

      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            type="button"
            onclick={() => onToggleTimeline?.()}
            aria-label="Toggle timeline"
            aria-pressed={showTimeline}
            class={toggleClass(showTimeline)}
          >
            <PanelBottom size={12} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            {showTimeline ? "Hide timeline" : "Show timeline"}
            <Kbd>⌘J</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <button
            type="button"
            onclick={() => onToggleSidebar?.()}
            aria-label="Toggle properties panel"
            aria-pressed={showSidebar}
            class={toggleClass(showSidebar)}
          >
            <PanelRight size={12} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            {showSidebar ? "Hide properties" : "Show properties"}
            <Kbd>⌘B</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>
    </div>


  </div>
</div>

<PresetPicker
  open={showPresetsPicker}
  onOpenChange={(v) => (showPresetsPicker = v)}
  onapply={applyPreset}
  currentId={store.lastAppliedPresetId}
/>

<ConfirmDialog
  bind:open={showRevertConfirm}
  onOpenChange={(v) => (showRevertConfirm = v)}
  title="Revert unsaved changes?"
  description="Restores every setting to the last save. You can undo the revert with Ctrl+Z."
  confirmLabel="Revert"
  cancelLabel="Keep editing"
  variant="destructive"
  onConfirm={() => store.revertToSaved()}
/>
