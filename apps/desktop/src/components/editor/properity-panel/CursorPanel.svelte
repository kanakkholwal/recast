<script lang="ts">
  import { SMOOTHING_PRESETS } from "$lib/cursor/smoothing";
  import { CURSOR_STYLES } from "$lib/cursor/styles";
  import { EASE } from "$lib/easing/cubic-bezier";
  import type {
    CursorStyleId,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    Activity,
    Eye,
    EyeOff,
    GitGraph,
    MousePointer,
    Sparkles,
    Target,
    Waves,
    Wind,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";
  import BezierEditor from "../_components/BezierEditor.svelte";
  import CursorTrajectoryMap from "../_components/CursorTrajectoryMap.svelte";
  import { SliderControl } from "@recast/ui/slider-control";
  import InspectorHint from "../InspectorHint.svelte";
  import PanelSection from "./PanelSection.svelte";

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
  let showTrajectoryMap = $state(false);

  const activeStyle = $derived(
    CURSOR_STYLES.find((s) => s.id === store.cursorSettings.style),
  );

  function updateCursorSettings(
    updates: Partial<EditorStore["cursorSettings"]>,
    trackUndo = false,
  ) {
    if (trackUndo) store.pushUndoState();
    store.updateCursorSettings(updates);
  }
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- Visibility toggle (no section title — panel name is shown in the header) -->
  <div class="flex items-center justify-between gap-2 rounded-md border border-border/60 bg-card/40 px-2.5 py-1.5">
    <span class="text-[11px] text-muted-foreground">
      Tune how the captured pointer feels during playback.
    </span>
    <Button
      variant={store.cursorSettings.enabled ? "default_soft" : "outline"}
      size="xs"
      class="gap-1.5"
      onclick={() =>
        updateCursorSettings(
          { enabled: !store.cursorSettings.enabled },
          true,
        )}
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

  {#if store.cursorSettings.enabled}
    <PanelSection
      title="Style"
      hint="Pick a cursor sprite. The default soft dot ships through both preview and export. Other styles show in the editor preview today; export still uses the soft dot until the cursor sprite raster lands in the export overlay."
      flush
    >
      {#snippet action()}
        {#if activeStyle}
          <span class="font-mono text-[10px] tracking-tight text-foreground/80">
            {activeStyle.label}
          </span>
        {/if}
      {/snippet}
      <div
        class="grid grid-cols-5 gap-1 rounded-lg border border-border/60 bg-muted/30 p-1 shadow-(--shadow-craft-inset)"
      >
        {#each CURSOR_STYLES as style, i (style.id)}
          {@const isActive = store.cursorSettings.style === style.id}
          <button
            in:fly={{ y: 6, duration: 240, delay: 60 + i * 35, easing: cubicOut }}
            type="button"
            aria-pressed={isActive}
            aria-label={`${style.label} cursor`}
            onclick={() => {
              store.pushUndoState();
              store.updateCursorSettings({ style: style.id as CursorStyleId });
            }}
            title={`${style.label} — ${style.description}`}
            class={cn(
              "group relative aspect-square overflow-hidden rounded-md border transition-all duration-150",
              "focus:outline-none focus:ring-2 focus:ring-ring/40",
              "[&_svg]:h-[60%] [&_svg]:w-[60%]",
              isActive
                ? "border-primary/60 bg-primary/8 text-foreground"
                : "border-transparent bg-background/40 text-foreground/80 hover:border-border hover:bg-background/80 hover:text-foreground",
            )}
          >
            <span
              class={cn(
                "absolute inset-0 flex items-center justify-center transition-transform duration-150",
                isActive
                  ? "scale-100"
                  : "scale-90 opacity-85 group-hover:scale-100 group-hover:opacity-100",
              )}
              aria-hidden="true"
            >
              {@html style.svg}
            </span>
            {#if isActive}
              <span
                aria-hidden="true"
                class="pointer-events-none absolute right-0.5 top-0.5 size-1.5 rounded-full bg-primary shadow-[0_0_0_1.5px_color-mix(in_srgb,var(--color-background)_85%,transparent)]"
              ></span>
            {/if}
          </button>
        {/each}
      </div>

      {#if activeStyle}
        <p
          class="mt-1.5 line-clamp-2 text-[10px] leading-snug text-muted-foreground"
        >
          {activeStyle.description}
        </p>
      {/if}
    </PanelSection>

    <PanelSection
      title="Pointer"
      hint="Size controls how legibly the cursor reads on screen."
    >
      {#snippet action()}
        {#if store.cursorSettings.size !== 2}
          <Button
            variant="ghost"
            size="xs"
            onclick={() => updateCursorSettings({ size: 2 }, true)}
          >
            Reset
          </Button>
        {/if}
      {/snippet}
      <SliderControl
        label="Cursor size"
        value={store.cursorSettings.size}
        min={1}
        max={15}
        step={1}
        unit="x"
        onstart={() => store.pushUndoState()}
        onchange={(next) => store.updateCursorSettings({ size: next })}
      >
        {#snippet icon()}
          <MousePointer size={11} />
        {/snippet}
      </SliderControl>
    </PanelSection>

    <PanelSection
      title="Animation"
      hint="Cinematic touches applied at export. Bounce reacts to clicks, sway adds subtle life at rest, motion blur trails the cursor during fast movement."
    >
      {#snippet action()}
        {#if store.cursorSettings.clickBounce !== 0 || store.cursorSettings.sway !== 0 || store.cursorSettings.motionBlur !== 0 || store.cursorSettings.bounceSpeedMs !== 220}
          <Button
            variant="ghost"
            size="xs"
            onclick={() =>
              updateCursorSettings(
                {
                  clickBounce: 0,
                  sway: 0,
                  motionBlur: 0,
                  bounceSpeedMs: 220,
                },
                true,
              )}
            title="Reset all animation knobs"
          >
            Reset
          </Button>
        {/if}
      {/snippet}
        <span
          class="block"
          in:fly={{ y: 4, duration: 220, delay: 60, easing: cubicOut }}
        >
          <SliderControl
            label="Click bounce"
            description="How much the cursor squashes when you click"
            value={store.cursorSettings.clickBounce}
            min={0}
            max={5}
            step={0.05}
            unit="x"
            onstart={() => store.pushUndoState()}
            onchange={(next) =>
              store.updateCursorSettings({ clickBounce: next })}
          >
            {#snippet icon()}
              <Activity size={11} />
            {/snippet}
          </SliderControl>
        </span>

        {#if store.cursorSettings.clickBounce > 0}
          <span
            class="block"
            in:fly={{ y: 4, duration: 200, easing: cubicOut }}
          >
            <SliderControl
              label="Bounce speed"
              description="Length of the bounce window"
              value={store.cursorSettings.bounceSpeedMs}
              min={80}
              max={500}
              step={10}
              unit=" ms"
              onstart={() => store.pushUndoState()}
              onchange={(next) =>
                store.updateCursorSettings({ bounceSpeedMs: next })}
            >
              {#snippet icon()}
                <Waves size={11} />
              {/snippet}
            </SliderControl>
          </span>
        {/if}

        <span
          class="block"
          in:fly={{ y: 4, duration: 220, delay: 120, easing: cubicOut }}
        >
          <SliderControl
            label="Cursor sway"
            description="Subtle wobble during slow motion — disappears as you move faster"
            value={store.cursorSettings.sway}
            min={0}
            max={1}
            step={0.01}
            unit="x"
            onstart={() => store.pushUndoState()}
            onchange={(next) => store.updateCursorSettings({ sway: next })}
          >
            {#snippet icon()}
              <Wind size={11} />
            {/snippet}
          </SliderControl>
        </span>

        <span
          class="block"
          in:fly={{ y: 4, duration: 220, delay: 180, easing: cubicOut }}
        >
          <SliderControl
            label="Motion blur"
            description="Velocity-proportional trail behind fast cursor movement"
            value={store.cursorSettings.motionBlur}
            min={0}
            max={1}
            step={0.01}
            unit="x"
            onstart={() => store.pushUndoState()}
            onchange={(next) =>
              store.updateCursorSettings({ motionBlur: next })}
          >
            {#snippet icon()}
              <Sparkles size={11} />
            {/snippet}
          </SliderControl>
        </span>
    </PanelSection>

    <PanelSection
      title="Smoothing"
      hint="Gaussian-window smoothing over the captured mouse path. The click-snap option anchors the smoothed curve to the exact press position inside the snap window so buttons still get hit cleanly."
      flush
    >
      {#snippet action()}
        <Button
          size="icon-xs"
          variant="raw"
          title="Toggle trajectory map"
          aria-pressed={showTrajectoryMap}
          onclick={() => (showTrajectoryMap = !showTrajectoryMap)}
        >
          <GitGraph size={11} class="text-muted-foreground" />
        </Button>
      {/snippet}

      <div class="flex flex-col gap-2.5">
        {#if showTrajectoryMap}
          <CursorTrajectoryMap
            samples={store.cursorSamplesRaw}
            videoWidth={store.metadata?.width ?? 0}
            videoHeight={store.metadata?.height ?? 0}
            smoothing={store.cursorSettings.smoothing}
            snapToClicks={store.cursorSettings.snapToClicks}
            snapWindowMs={store.cursorSettings.snapWindowMs}
          />
        {/if}

        <!-- Presets -->
        <div class="flex flex-wrap gap-1">
          {#each SMOOTHING_PRESETS as preset, i (preset.id)}
            {@const isActive =
              store.cursorSettings.smoothing === preset.smoothing &&
              store.cursorSettings.snapToClicks === preset.snapToClicks &&
              store.cursorSettings.snapWindowMs === preset.snapWindowMs}
            <span
              class="inline-flex"
              in:scale={{ start: 0.92, duration: 220, delay: 80 + i * 30, easing: cubicOut }}
            >
              <Button
                type="button"
                aria-pressed={isActive}
                onclick={() => {
                  store.pushUndoState();
                  store.updateCursorSettings({
                    smoothing: preset.smoothing,
                    snapToClicks: preset.snapToClicks,
                    snapWindowMs: preset.snapWindowMs,
                  });
                }}
                size="xs"
                variant={isActive ? "default_soft" : "outline"}
              >
                {preset.label}
              </Button>
            </span>
          {/each}
        </div>

        <div class="space-y-2.5">
        <SliderControl
          label="Smoothing"
          value={store.cursorSettings.smoothing}
          min={0}
          max={100}
          step={5}
          unit="%"
          description={store.cursorSettings.smoothing === 0
            ? "Off — cursor follows the raw capture"
            : undefined}
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCursorSettings({ smoothing: next })}
        >
          {#snippet icon()}
            <Sparkles size={11} />
          {/snippet}
        </SliderControl>

        <div
          class="flex items-center justify-between gap-2 rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur px-2 py-1.5"
        >
          <div class="flex items-center gap-1.5">
            <Target size={11} class="text-muted-foreground" />
            <span class="text-[11px] font-medium text-foreground"
              >Snap to clicks</span
            >
            <InspectorHint
              content="Around every mouse-down, pin the smoothed curve to the exact click x/y inside the snap window. Prevents smoothing from rounding the corner off a press target."
            />
          </div>
          <Button
            variant={store.cursorSettings.snapToClicks
              ? "default_soft"
              : "outline"}
            size="xs"
            aria-pressed={store.cursorSettings.snapToClicks}
            onclick={() =>
              updateCursorSettings(
                { snapToClicks: !store.cursorSettings.snapToClicks },
                true,
              )}
          >
            {store.cursorSettings.snapToClicks ? "On" : "Off"}
          </Button>
        </div>

        {#if store.cursorSettings.snapToClicks}
          <SliderControl
            label="Snap window"
            value={store.cursorSettings.snapWindowMs}
            min={0}
            max={200}
            step={10}
            unit="ms"
            description="Half-width of the cosine-ramped anchor around each click."
            onstart={() => store.pushUndoState()}
            onchange={(next) =>
              store.updateCursorSettings({ snapWindowMs: next })}
          >
            {#snippet icon()}
              <Target size={11} />
            {/snippet}
          </SliderControl>
        {/if}
        </div>
      </div>
    </PanelSection>

    <PanelSection
      title="Motion easing"
      hint="Reshape how the cursor interpolates between captured samples. Default (linear) preserves the raw trajectory. Ease-out curves decelerate into rest for a more deliberate feel."
      flush
    >
      {#snippet action()}
        <Button
          variant={store.cursorMotionEasing ? "default_soft" : "outline"}
          size="xs"
          class="gap-1.5"
          aria-pressed={!!store.cursorMotionEasing}
          onclick={() =>
            (store.cursorMotionEasing = store.cursorMotionEasing
              ? null
              : { ...EASE })}
        >
          <Waves size={11} />
          {store.cursorMotionEasing ? "On" : "Off"}
        </Button>
      {/snippet}
      {#if store.cursorMotionEasing}
        <BezierEditor
          value={store.cursorMotionEasing}
          onchange={(next) => (store.cursorMotionEasing = next)}
          description="Applies to preview only"
          size={160}
        />
      {/if}
    </PanelSection>

    <PanelSection
      title="Click highlight"
      hint="Useful for tutorials and product demos where click targets should be obvious."
      flush
    >
      {#snippet action()}
        <Button
          variant={store.cursorSettings.highlightClicks
            ? "default_soft"
            : "outline"}
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
      {/snippet}

      {#if store.cursorSettings.highlightClicks}
        <div class="grid grid-cols-8 gap-1" in:fade={{ duration: 160 }}>
          {#each highlightColors as color, i (color)}
            {@const isSelected = store.cursorSettings.highlightColor === color}
            <span
              class="inline-flex"
              in:scale={{ start: 0.85, duration: 220, delay: 60 + i * 25, easing: cubicOut }}
            >
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
                  "aspect-square w-full rounded-md border-2 transition-all",
                  isSelected
                    ? "border-foreground shadow-sm"
                    : "border-border/40 hover:border-border",
                )}
                style="background-color: {color}"
              ></Button>
            </span>
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
            onchange={(next) =>
              store.updateCursorSettings({ highlightOpacity: next })}
          />
        </div>
      {/if}
    </PanelSection>

    <PanelSection
      title="Idle"
      hint="Hide the cursor after inactivity for cleaner sections without interaction."
      flush
    >
      {#snippet action()}
        <Button
          variant={store.cursorSettings.hideWhenIdle
            ? "default_soft"
            : "outline"}
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
      {/snippet}
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
    </PanelSection>
  {:else}
    <div
      class="flex items-center gap-2 rounded-md border border-dashed border-border bg-muted/20 px-3 py-2.5"
    >
      <EyeOff size={13} class="shrink-0 text-muted-foreground" />
      <p class="flex-1 text-[11px] text-muted-foreground">
        Cursor is hidden. Enable it to tune size, smoothing and click
        highlights.
      </p>
    </div>
  {/if}
</div>
