<script lang="ts">
  import {
    EASE,
    easingEquals,
    type Easing,
  } from "$lib/easing/cubic-bezier";
  import { registry } from "$lib/registry";
  import { clockCentis as fmtTime } from "$lib/format/time";
  import {
    computeNewZoomBounds,
    regionMaxRamp,
    scaleAt,
    sparklinePath,
  } from "./focus-panel.logic";
  import {
    DEFAULT_ZOOM_CENTER,
    DEFAULT_ZOOM_RAMP,
    type EditorStore,
    type ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import { resolveZoomCenter } from "$lib/zoom/auto-apply";
  import {
    AiBrain,
    AiWand,
    Clock,
    Copy,
    Crosshair,
    Eye,
    EyeOff,
    MoveHorizontal,
    MoveVertical,
    Plus,
    Sparkles,
    TrendingDown,
    TrendingUp,
    Trash2,
    Wind,
    ZoomIn,
  } from "@recast/icons";
  import { motionDuration } from "$lib/motion.svelte";
  import { Button } from "@recast/ui/button";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import BezierEditor from "../_components/BezierEditor.svelte";
  import InspectorHint from "../InspectorHint.svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  // Built-in + extension easing presets, from the registry.
  const easingPresets = $derived(
    registry
      .list("easing")
      .map((e) => ({ id: e.id, label: e.label, value: e.value.value })),
  );

  const selected = $derived<ZoomRegion | null>(
    store.zoomRegions.find((r) => r.id === store.selectedZoomRegionId) ?? null,
  );

  // Listed in timeline order (by start time) so the panel scans the same way the
  // timeline reads, left to right, and numbered so a row correlates with the
  // "Region N" detail header.
  const orderedRegions = $derived(
    [...store.zoomRegions].sort((a, b) => a.start - b.start),
  );
  const selectedIndex = $derived(
    selected ? orderedRegions.findIndex((r) => r.id === selected.id) : -1,
  );

  // Which ramp the Custom-curves editor targets (one graph at a time).
  let customCurve = $state<"in" | "out">("in");

  function addRegion() {
    const bounds = computeNewZoomBounds(
      store.metadata?.duration ?? 0,
      store.trimStart,
      store.trimEnd,
      store.currentTime,
    );
    if (!bounds) return;
    // Zoom toward where the cursor actually was, not dead-centre.
    const w = store.metadata?.width ?? 0;
    const h = store.metadata?.height ?? 0;
    const center = resolveZoomCenter(store.cursorSamplesRaw, store.currentTime, w, h);
    store.addZoomRegion(bounds.start, bounds.end, 1.8, center);
  }

  let hasAutoZooms = $derived(store.zoomRegions.some((r) => r.source === "auto"));

  function rerunAutoZoom() {
    window.dispatchEvent(new CustomEvent("recast:rerun-auto-zoom"));
  }

  function clearAuto() {
    store.clearAutoZooms();
  }

  function removeSelected() {
    if (!selected) return;
    store.removeZoomRegion(selected.id);
  }

  function clearAllRegions() {
    store.clearZoomRegions();
  }

  function updateSelected(updates: Partial<ZoomRegion>, trackUndo = false) {
    if (!selected) return;
    if (trackUndo) store.pushUndoState();
    store.updateZoomRegion(selected.id, updates);
  }

  function resetCurves() {
    if (!selected) return;
    store.pushUndoState();
    store.updateZoomRegion(selected.id, {
      easeIn: { ...EASE },
      easeOut: { ...EASE },
      rampIn: DEFAULT_ZOOM_RAMP,
      rampOut: DEFAULT_ZOOM_RAMP,
    });
  }

  // Recenters the focus point only. Motion blur is a separate control in the Zoom
  // section, so this button (in Focus point) must not silently reset it too.
  function recenterFocus() {
    if (!selected) return;
    store.pushUndoState();
    store.updateZoomRegion(selected.id, {
      centerX: DEFAULT_ZOOM_CENTER,
      centerY: DEFAULT_ZOOM_CENTER,
    });
  }

  function applyPresetToBoth(preset: Easing) {
    if (!selected) return;
    store.pushUndoState();
    store.updateZoomRegion(selected.id, {
      easeIn: { ...preset },
      easeOut: { ...preset },
    });
  }
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <PanelSection
    title="Regions"
    hint="Each region zooms the clip with its own ease-in / ease-out. Park the playhead where you want to zoom, then Add."
  >
    {#snippet action()}
      <div class="flex items-center gap-2">
        {#if store.zoomRegions.length > 0}
          <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
            {store.zoomRegions.length}
          </span>
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5 text-muted-foreground hover:text-destructive"
            onclick={clearAllRegions}
          >
            <Trash2 size={11} />
            Clear all
          </Button>
        {/if}
        <Button
          variant="secondary"
          size="xs"
          class="gap-1.5"
          onclick={addRegion}
          disabled={!store.metadata?.duration}
        >
          <Plus size={11} />
          Add
        </Button>
      </div>
    {/snippet}

    <!-- Smart Auto-Zoom, kept next to "Add" rather than below the list.
         On-import preference + an on-demand re-run. -->
    <div
      class="flex flex-col gap-2 rounded-xl border border-border/60 bg-card/70 px-2.5 py-2 shadow-(--shadow-craft-inset) backdrop-blur"
    >
      <div class="flex items-center gap-1.5">
        <AiBrain size={12} class="shrink-0 text-primary" />
        <span class="text-[11px] font-medium text-foreground">Smart Auto-Zoom</span>
        <InspectorHint
          content="Adds a focus moment at every click and settle point when a recording first opens."
        />
        <div class="ml-auto flex items-center gap-1.5">
          <span class="text-[10px] text-muted-foreground">On import</span>
          <SegmentedToggle
            checked={store.autoZoomEnabled}
            size="xs"
            aria-label="Smart auto-zoom on import"
            onCheckedChange={(next) => (store.autoZoomEnabled = next)}
          />
        </div>
      </div>
      <div class="flex items-center justify-between gap-2">
        <p class="text-[10px] leading-snug text-muted-foreground">
          Generate focus moments from cursor activity.
        </p>
        <div class="flex shrink-0 items-center gap-1">
          {#if hasAutoZooms}
            <Button variant="ghost" size="xs" onclick={clearAuto}>Clear</Button>
          {/if}
          <Button
            variant="secondary"
            size="xs"
            class="gap-1.5"
            onclick={rerunAutoZoom}
            disabled={!store.cursorPath}
          >
            <AiWand size={11} />
            Re-run
          </Button>
        </div>
      </div>
    </div>

    {#if store.zoomRegions.length === 0}
      <div
        class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border/70 bg-card/40 px-3 py-6 text-center"
      >
        <div
          class="flex size-9 items-center justify-center rounded-lg border border-border/60 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset)"
        >
          <ZoomIn size={16} />
        </div>
        <p class="text-[11px] font-medium text-foreground">No zoom regions yet</p>
        <p class="text-[10px] leading-snug text-muted-foreground">
          Park the playhead where you want to zoom, then press Add.
        </p>
      </div>
    {:else}
      <div class="flex flex-col gap-1">
        {#each orderedRegions as region, i (region.id)}
          {@const isActive = region.id === store.selectedZoomRegionId}
          {@const isHidden = region.hidden === true}
          <!-- Absolute-inset select button so the whole row picks the region,
               with action buttons on their own z-layer, since nesting <button>s
               would be invalid markup. -->
          <div
            in:fly={{
              y: 4,
              duration: motionDuration(200),
              delay: motionDuration(i * 25),
              easing: cubicOut,
            }}
            class={cn(
              "group relative flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-2 text-left transition-all duration-150",
              isActive
                ? "border-primary/60 bg-primary/10 shadow-(--shadow-craft-inset)"
                : "border-border/60 bg-card/60 hover:border-border hover:bg-card",
              isHidden && "opacity-55",
            )}
          >
            <button
              type="button"
              onclick={() => (store.selectedZoomRegionId = region.id)}
              aria-pressed={isActive}
              aria-label={`Zoom region ${i + 1}: ${region.scale.toFixed(1)}× at ${fmtTime(region.start)}`}
              class="absolute inset-0 z-0 rounded-lg focus:outline-none focus:ring-2 focus:ring-ring/40"
            ></button>
            <span
              class={cn(
                "pointer-events-none w-3.5 shrink-0 text-center text-[10px] font-semibold tabular-nums",
                isActive ? "text-primary" : "text-muted-foreground/70",
              )}>{i + 1}</span>
            <span
              class={cn(
                "pointer-events-none flex h-8 w-12 shrink-0 items-center justify-center rounded-md border transition-colors",
                isActive
                  ? "border-primary/40 bg-background/40 text-primary"
                  : "border-border/50 bg-background/40 text-muted-foreground group-hover:text-foreground",
              )}
            >
              <svg viewBox="0 0 100 18" width="40" height="13" aria-hidden="true">
                <path
                  d={sparklinePath(region, 100, 18)}
                  stroke="currentColor"
                  stroke-width="1.6"
                  fill="none"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
            <div class="pointer-events-none min-w-0 flex-1">
              <div class="flex items-center gap-1.5">
                <span
                  class="truncate text-[11px] font-medium tabular-nums text-foreground"
                >
                  {region.scale.toFixed(2)}× · {fmtTime(region.start)}–{fmtTime(
                    region.end,
                  )}
                </span>
                {#if region.source === "auto"}
                  <span
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-primary/30 bg-primary/10 px-1 text-[9px] font-semibold uppercase tracking-wider text-primary"
                  >
                    <Sparkles size={8} />
                    Auto
                  </span>
                {/if}
                {#if isHidden}
                  <span
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-border bg-muted/60 px-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground"
                  >
                    <EyeOff size={8} />
                    Hidden
                  </span>
                {/if}
              </div>
              <div class="text-[10px] tabular-nums text-muted-foreground">
                {(region.end - region.start).toFixed(2)}s duration
              </div>
            </div>
            <!-- Row actions on hover/focus only. For the SELECTED row these
                 would just duplicate the detail header's Hide/Duplicate/Delete
                 (right below), so master-detail hands the selected item's
                 actions to the header and the row stays quiet unless hovered. -->
            <div
              class="relative z-10 flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
            >
              <button
                type="button"
                onclick={() => store.setZoomRegionHidden(region.id)}
                aria-label={isHidden ? "Show region" : "Hide region"}
                title={isHidden ? "Show" : "Hide"}
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring/40"
              >
                {#if isHidden}
                  <EyeOff size={12} />
                {:else}
                  <Eye size={12} />
                {/if}
              </button>
              <button
                type="button"
                onclick={() => store.duplicateZoomRegion(region.id)}
                aria-label="Duplicate region"
                title="Duplicate"
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring/40"
              >
                <Copy size={12} />
              </button>
              <button
                type="button"
                onclick={() => store.removeZoomRegion(region.id)}
                aria-label="Delete region"
                title="Delete"
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/15 hover:text-destructive focus:outline-none focus:ring-2 focus:ring-ring/40"
              >
                <Trash2 size={12} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </PanelSection>

  <!-- Region editor (master-detail) with a header showing which region the
       controls edit. -->
  {#if selected}
    {@const region = selected}
    {@const maxRamp = regionMaxRamp(region)}
    <div
      in:fly={{ y: 6, duration: motionDuration(200), easing: cubicOut }}
      class="flex flex-col gap-3 border-t border-border/50 pt-3"
    >
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <p class="text-[11px] font-semibold tracking-tight text-foreground">
            Region {selectedIndex + 1}
          </p>
          <p class="truncate text-[10px] tabular-nums text-muted-foreground">
            {region.scale.toFixed(2)}× · {fmtTime(region.start)}–{fmtTime(
              region.end,
            )}
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-1.5">
          <Button
            variant="outline"
            size="xs"
            class="gap-1.5"
            onclick={() => store.setZoomRegionHidden(region.id)}
          >
            {#if region.hidden}
              <EyeOff size={11} />
              Hidden
            {:else}
              <Eye size={11} />
              Hide
            {/if}
          </Button>
          <Button
            variant="outline"
            size="xs"
            class="gap-1.5"
            onclick={() => store.duplicateZoomRegion(region.id)}
          >
            <Copy size={11} />
            Duplicate
          </Button>
          <Button
            variant="destructive_soft"
            size="xs"
            class="gap-1.5"
            onclick={removeSelected}
          >
            <Trash2 size={11} />
            Delete
          </Button>
        </div>
      </div>

      <PanelSection title="Zoom">
        <SliderControl
          label="Scale"
          value={region.scale}
          min={1}
          max={3}
          step={0.05}
          unit="×"
          formatValue={(v) => `${v.toFixed(2)}×`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ scale: v })}
        >
          {#snippet icon()}
            <ZoomIn size={11} />
          {/snippet}
        </SliderControl>
        <SliderControl
          label="Motion blur"
          value={Math.round(region.motionBlur * 100)}
          min={0}
          max={100}
          step={1}
          unit="%"
          formatValue={(v) => `${v.toFixed(0)}%`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ motionBlur: v / 100 })}
        >
          {#snippet icon()}
            <Wind size={11} />
          {/snippet}
        </SliderControl>
      </PanelSection>

      <PanelSection
        title="Focus point"
        hint="Drag the rectangle on the preview, or use the sliders. Values are 0..1 across the frame (0.5 = center)."
      >
        {#snippet action()}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5"
            onclick={recenterFocus}
            disabled={region.centerX === 0.5 && region.centerY === 0.5}
          >
            <Crosshair size={11} />
            Recenter
          </Button>
        {/snippet}
        <SliderControl
          label="Focus X"
          value={region.centerX}
          min={0}
          max={1}
          step={0.01}
          formatValue={(v) => v.toFixed(2)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ centerX: v })}
        >
          {#snippet icon()}
            <MoveHorizontal size={11} />
          {/snippet}
        </SliderControl>
        <SliderControl
          label="Focus Y"
          value={region.centerY}
          min={0}
          max={1}
          step={0.01}
          formatValue={(v) => v.toFixed(2)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ centerY: v })}
        >
          {#snippet icon()}
            <MoveVertical size={11} />
          {/snippet}
        </SliderControl>
      </PanelSection>

      <PanelSection
        title="Timing"
        hint="When the region runs and how long it ramps in and out. Use split ramps to hold at full zoom before releasing."
      >
        <SliderControl
          label="Start"
          value={region.start}
          min={0}
          max={Math.max(region.end - 0.1, 0)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ start: v })}
        >
          {#snippet icon()}
            <Clock size={11} />
          {/snippet}
        </SliderControl>
        <SliderControl
          label="End"
          value={region.end}
          min={region.start + 0.1}
          max={store.metadata?.duration ?? region.end}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ end: v })}
        >
          {#snippet icon()}
            <Clock size={11} />
          {/snippet}
        </SliderControl>
        <SliderControl
          label="Ramp in"
          value={region.rampIn}
          min={0}
          max={Math.max(maxRamp, 0.01)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampIn: v })}
        >
          {#snippet icon()}
            <TrendingUp size={11} />
          {/snippet}
        </SliderControl>
        <SliderControl
          label="Ramp out"
          value={region.rampOut}
          min={0}
          max={Math.max(maxRamp, 0.01)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampOut: v })}
        >
          {#snippet icon()}
            <TrendingDown size={11} />
          {/snippet}
        </SliderControl>
      </PanelSection>

      <!-- Presets lead; raw bezier curves live behind a "Custom curves" disclosure. -->
      <PanelSection title="Easing" hint="How the zoom accelerates in and decelerates out.">
        {#snippet action()}
          <Button variant="ghost" size="xs" onclick={resetCurves}>Reset</Button>
        {/snippet}
        <div class="flex flex-wrap gap-1">
          {#each easingPresets as preset (preset.id)}
            {@const active =
              easingEquals(region.easeIn, preset.value) &&
              easingEquals(region.easeOut, preset.value)}
            <Button
              type="button"
              size="xs"
              aria-pressed={active}
              variant={active ? "default_soft" : "outline"}
              onclick={() => applyPresetToBoth(preset.value)}
            >
              {preset.label}
            </Button>
          {/each}
        </div>

        <PanelSection title="Custom curves" flush collapsible defaultOpen={false}>
          <div class="flex flex-col gap-2 pt-1">
            <!-- One editor at a time, switched in/out; the card's sparkline
                 previews the combined result. -->
            <div class="flex items-center justify-between gap-2">
              <div class="flex items-center gap-1.5">
                <span class="text-[10px] font-medium text-muted-foreground">
                  Editing the {customCurve === "in" ? "ease-in" : "ease-out"} ramp
                </span>
                <InspectorHint
                  content="Drag the two handles to shape this ramp. Switch between the ease-in and ease-out curves with the toggle."
                />
              </div>
              <SegmentedToggle
                checked={customCurve === "out"}
                offLabel="In"
                onLabel="Out"
                size="xs"
                aria-label="Edit ease-in or ease-out curve"
                onCheckedChange={(next) => (customCurve = next ? "out" : "in")}
              />
            </div>
            <BezierEditor
              value={customCurve === "in" ? region.easeIn : region.easeOut}
              onchange={(v) =>
                updateSelected(
                  customCurve === "in" ? { easeIn: v } : { easeOut: v },
                  true,
                )}
              showPresets={false}
              size={220}
            />
          </div>
        </PanelSection>
      </PanelSection>
    </div>
  {/if}
</div>
