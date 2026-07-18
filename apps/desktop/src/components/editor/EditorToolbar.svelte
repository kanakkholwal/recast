<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    ArrowLeft,
    LoaderCircle,
    Maximize2,
    Minimize2,
    PanelBottom,
    PanelRight,
    RotateCcw,
    Save,
    Sparkles,
    Upload,
    X,
  } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { Separator } from "@recast/ui/separator";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import ConfirmDialog from "../recast/ConfirmDialog.svelte";
  import PresetPicker, { PRESETS, type Preset } from "./PresetPicker.svelte";
  import { onMount } from "svelte";
  import {
    chordLabel,
    registerShortcutHandlers,
  } from "$lib/shortcuts/registry.svelte";

  interface Props {
    store: EditorStore;
    filename?: string;
    onexport?: () => void;
    onsave?: () => void | Promise<void>;
    isSaving?: boolean;
    // Drives the Export button's label/icon/action:
    //   export   idle, opens the export surface
    //   close    options picker is open, closes it
    //   minimize export surface is foregrounded, sends it to the activity center
    //   show     export is running/finished but minimized, reopens it
    exportMode?: "export" | "close" | "minimize" | "show";
    /** Whether this editor's export is actively encoding (for the minimized
     *  "Exporting…" label). */
    exportRunning?: boolean;
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
    exportMode = "export",
    exportRunning = false,
    showSidebar = true,
    showTimeline = true,
    onToggleSidebar,
    onToggleTimeline,
  }: Props = $props();

  // The panel/timeline toggles are meaningless while the export surface owns the
  // layout, so they're disabled then rather than silently doing nothing. (A
  // minimized export is back in the normal editing layout, so they stay live.)
  const exportOpen = $derived(
    exportMode === "close" || exportMode === "minimize",
  );

  const toggleClass = (active: boolean) =>
    cn(
      "cursor-pointer flex size-6 items-center justify-center rounded-md transition-colors duration-150",
      "disabled:pointer-events-none disabled:opacity-40",
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
        // The export surface owns the rail and its own Esc routing. Opening the
        // picker over it strands Esc between two handlers, and the export one
        // cancels the render. Every other editor chord already bails here.
        if (exportOpen) return;
        showPresetsPicker = !showPresetsPicker;
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

  // The action (open / close / minimize / show) is decided by the parent from
  // exportMode; this just forwards the click.
  function onExportClick() {
    onexport?.();
  }
</script>

<div
  class="flex h-full w-full items-center gap-1.5 px-2 text-[11px]"
  
>
  <!-- Every Tooltip.Trigger below uses `child` to merge its props onto our own
       element. Without it the trigger renders a button AROUND the control:
       nested interactive elements, two tab stops each, and a disabled control
       whose tooltip still opens (the pointer falls through to the wrapper). -->
  <div class="flex items-center gap-0.5">
    <Tooltip.Root>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant="ghost"
            size="icon-sm"
            href="/recasts"
            aria-label="Back"
          >
            <ArrowLeft size={12} />
          </Button>
        {/snippet}
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
        {#snippet child({ props })}
          <Button
            {...props}
            variant="ghost"
            size="xs"
            class="gap-1.5 text-[11px] text-muted-foreground"
            onclick={() => (showPresetsPicker = true)}
          >
            <Sparkles size={12} />
            Presets
            <Kbd class="ml-1">{chordLabel("editor.presets")}</Kbd>
          </Button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content>Browse social & studio presets</Tooltip.Content>
    </Tooltip.Root>

    {#if activePreset || store.outputAspect !== "source"}
      <div
        class="flex h-6 items-center gap-1 rounded-md border border-primary/30 bg-primary/10 pl-1.5 pr-0.5 text-[11px] font-semibold text-primary"
      >
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                onclick={() => (showPresetsPicker = true)}
                class="flex h-full items-center gap-1.5 cursor-pointer"
                aria-label="Change preset"
              >
                {#if activePreset}
                  <span
                    class="text-[10px] uppercase tracking-wider text-primary/70"
                  >
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
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>Change preset</Tooltip.Content>
        </Tooltip.Root>
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                onclick={clearPreset}
                aria-label="Reset to source aspect"
                class="ml-0.5 flex size-5 cursor-pointer items-center justify-center rounded text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary"
              >
                <X size={10} stroke={2.5} />
              </button>
            {/snippet}
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
          {#snippet child({ props })}
            <Button
              {...props}
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
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {isSaving
            ? "Saving. Wait for it to finish."
            : "Discard unsaved changes and restore the last saved state"}
        </Tooltip.Content>
      </Tooltip.Root>
    {/if}

    <Tooltip.Root>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant={store.isDirty ? "secondary" : "ghost"}
            size="xs"
            class="gap-1.5 text-[11px]"
            onclick={() => onsave?.()}
            disabled={isSaving || (!store.isDirty && !isSaving)}
            aria-label={store.isDirty ? "Save project" : "Project saved"}
          >
            {#if isSaving}
              <LoaderCircle size={12} class="animate-spin" />
              Saving…
            {:else}
              <Save size={12} />
              {store.isDirty ? "Save" : "Saved"}
            {/if}
          </Button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content>
        {#if store.isDirty}
          <span class="inline-flex items-center gap-1.5">
            Save project <Kbd>{chordLabel("editor.save")}</Kbd>
          </span>
        {:else}
          No unsaved changes
        {/if}
      </Tooltip.Content>
    </Tooltip.Root>

    {#if exportMode === "close"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        aria-pressed="true"
        class="gap-1.5 text-[11px]"
      >
        <X size={12} />
        Close
      </Button>
    {:else if exportMode === "minimize"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        aria-pressed="true"
        class="gap-1.5 text-[11px]"
      >
        <Minimize2 size={12} />
        Minimize
      </Button>
    {:else if exportMode === "show"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        class="gap-1.5 text-[11px]"
      >
        {#if exportRunning}
          <LoaderCircle size={12} class="animate-spin" />
          Exporting…
        {:else}
          <Maximize2 size={12} />
          Show export
        {/if}
      </Button>
    {:else}
      <Button onclick={onExportClick} size="xs" class="gap-1.5 text-[11px]">
        <Upload size={12} />
        Export
      </Button>
    {/if}
        <Separator orientation="vertical" class="mx-0.5 h-3.5" />

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              onclick={() => onToggleTimeline?.()}
              disabled={exportOpen}
              aria-label="Toggle timeline"
              aria-pressed={!exportOpen && showTimeline}
              class={toggleClass(showTimeline)}
            >
              <PanelBottom size={12} />
            </button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {#if exportOpen}
            Unavailable while the export panel is open
          {:else}
            <span class="inline-flex items-center gap-1.5">
              {showTimeline ? "Hide timeline" : "Show timeline"}
              <Kbd>{chordLabel("editor.toggleTimeline")}</Kbd>
            </span>
          {/if}
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              onclick={() => onToggleSidebar?.()}
              disabled={exportOpen}
              aria-label="Toggle properties panel"
              aria-pressed={!exportOpen && showSidebar}
              class={toggleClass(showSidebar)}
            >
              <PanelRight size={12} />
            </button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {#if exportOpen}
            Unavailable while the export panel is open
          {:else}
            <span class="inline-flex items-center gap-1.5">
              {showSidebar ? "Hide properties" : "Show properties"}
              <Kbd>{chordLabel("editor.toggleSidebar")}</Kbd>
            </span>
          {/if}
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
