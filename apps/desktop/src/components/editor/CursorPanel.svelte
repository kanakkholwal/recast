<script lang="ts">
  import { EASE } from "$lib/easing/cubic-bezier";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    Activity,
    Eye,
    EyeOff,
    MousePointer,
    Sparkles,
    Waves,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import BezierEditor from "./BezierEditor.svelte";
  import InspectorHint from "./InspectorHint.svelte";
  import SliderControl from "./SliderControl.svelte";

  interface Props {
    store: EditorStore;
  }

  // Semantic-token accents — no hardcoded hex beyond the actual highlight swatches
  const highlightColors = [
    "#3b82f6",
    "#ef4444",
    "#22c55e",
    "#f59e0b",
    "#8b5cf6",
    "#ec4899",
    "#06b6d4",
    "#ffffff",
  ];

  let { store }: Props = $props();

  function updateCursorSettings(
    updates: Partial<EditorStore["cursorSettings"]>,
    trackUndo = false,
  ) {
    if (trackUndo) store.pushUndoState();
    store.updateCursorSettings(updates);
  }
</script>

<div class="flex flex-col gap-5 animate-in fade-in duration-200">
  <!-- Header row: label + enabled toggle -->
  <section>
    <div class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-1.5">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Cursor
        </h3>
        <InspectorHint content="These controls tune how the captured pointer feels during playback." />
      </div>
      <Button
        variant={store.cursorSettings.enabled ? "default_soft" : "outline"}
        size="xs"
        class="gap-1.5"
        onclick={() => updateCursorSettings({ enabled: !store.cursorSettings.enabled }, true)}
        aria-pressed={store.cursorSettings.enabled}
      >
        {#if store.cursorSettings.enabled}
          <Eye size={11} />
          Visible
        {:else}
          <EyeOff size={11} />
          Hidden
        {/if}
      </Button>
    </div>
  </section>

  {#if store.cursorSettings.enabled}
    <!-- Pointer feel -->
    <section>
      <header class="mb-2 flex items-center gap-1.5">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Pointer
        </h3>
        <InspectorHint content="Size changes legibility. Smoothing makes motion feel less jittery." />
      </header>
      <div class="space-y-2.5">
        <SliderControl
          label="Cursor size"
          value={store.cursorSettings.size}
          min={1}
          max={5}
          step={1}
          unit="x"
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCursorSettings({ size: next })}
        >
          {#snippet icon()}
            <MousePointer size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Motion smoothing"
          value={store.cursorSettings.smoothing}
          min={0}
          max={100}
          step={5}
          unit="%"
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCursorSettings({ smoothing: next })}
        >
          {#snippet icon()}
            <Sparkles size={11} />
          {/snippet}
        </SliderControl>
      </div>
    </section>

    <!-- Custom motion easing: reshapes the per-sample lerp in the preview. -->
    <section>
      <div class="mb-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5">
          <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Motion easing
          </h3>
          <InspectorHint
            content="Reshape how the cursor interpolates between captured samples. Default (linear) preserves the raw trajectory. Ease-out curves decelerate into rest for a more deliberate feel."
          />
        </div>
        <Button
          variant={store.cursorMotionEasing ? "default_soft" : "outline"}
          size="xs"
          class="gap-1.5"
          aria-pressed={!!store.cursorMotionEasing}
          onclick={() =>
            (store.cursorMotionEasing = store.cursorMotionEasing ? null : { ...EASE })}
        >
          <Waves size={11} />
          {store.cursorMotionEasing ? "On" : "Off"}
        </Button>
      </div>
      {#if store.cursorMotionEasing}
        <BezierEditor
          value={store.cursorMotionEasing}
          onchange={(next) => (store.cursorMotionEasing = next)}
          description="Applies to preview only"
          size={160}
        />
      {/if}
    </section>

    <!-- Click highlight -->
    <section>
      <div class="mb-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5">
          <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Click Highlight
          </h3>
          <InspectorHint content="Useful for tutorials and product demos where click targets should be obvious." />
        </div>
        <Button
          variant={store.cursorSettings.highlightClicks ? "default_soft" : "outline"}
          size="xs"
          class="gap-1.5"
          onclick={() =>
            updateCursorSettings(
              { highlightClicks: !store.cursorSettings.highlightClicks },
              true,
            )}
          aria-pressed={store.cursorSettings.highlightClicks}
        >
          <Activity size={11} />
          {store.cursorSettings.highlightClicks ? "On" : "Off"}
        </Button>
      </div>

      {#if store.cursorSettings.highlightClicks}
        <div class="grid grid-cols-8 gap-1">
          {#each highlightColors as color}
            {@const isSelected = store.cursorSettings.highlightColor === color}
            <Button
              variant="raw"
              size="raw"
              onclick={() =>
                updateCursorSettings(
                  { highlightColor: color },
                  store.cursorSettings.highlightColor !== color,
                )}
              aria-label="Use {color} click highlight color"
              aria-pressed={isSelected}
              class={cn(
                "aspect-square rounded-md border-2 transition-all",
                isSelected
                  ? "border-foreground shadow-sm"
                  : "border-border/40 hover:border-border",
              )}
              style="background-color: {color}"
            ></Button>
          {/each}
        </div>

        <div class="mt-2.5">
          <SliderControl
            label="Highlight opacity"
            value={store.cursorSettings.highlightOpacity}
            min={10}
            max={100}
            step={5}
            unit="%"
            onstart={() => store.pushUndoState()}
            onchange={(next) => store.updateCursorSettings({ highlightOpacity: next })}
          />
        </div>
      {/if}
    </section>

    <!-- Idle behavior -->
    <section>
      <div class="mb-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5">
          <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Idle Behavior
          </h3>
          <InspectorHint content="Hide the cursor after inactivity for cleaner sections without interaction." />
        </div>
        <Button
          variant={store.cursorSettings.hideWhenIdle ? "default_soft" : "outline"}
          size="xs"
          onclick={() =>
            updateCursorSettings(
              { hideWhenIdle: !store.cursorSettings.hideWhenIdle },
              true,
            )}
          aria-pressed={store.cursorSettings.hideWhenIdle}
        >
          {store.cursorSettings.hideWhenIdle ? "On" : "Off"}
        </Button>
      </div>

      {#if store.cursorSettings.hideWhenIdle}
        <SliderControl
          label="Idle timeout"
          value={store.cursorSettings.idleTimeout}
          min={1}
          max={10}
          step={1}
          unit="s"
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCursorSettings({ idleTimeout: next })}
        />
      {/if}
    </section>
  {:else}
    <div
      class="flex items-center gap-2 rounded-md border border-dashed border-border bg-muted/20 px-3 py-2.5"
    >
      <EyeOff size={13} class="shrink-0 text-muted-foreground" />
      <p class="flex-1 text-[11px] text-muted-foreground">
        Cursor is hidden. Enable it to tune size, smoothing and click highlights.
      </p>
    </div>
  {/if}
</div>
