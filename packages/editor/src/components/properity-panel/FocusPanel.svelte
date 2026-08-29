<script lang="ts">
import {
	AiWand,
	Copy,
	Crosshair,
	Eye,
	EyeOff,
	Plus,
	Sparkles,
	Trash2,
	TriangleAlert,
	ZoomIn,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { SegmentedToggle } from "@recast/ui/segmented";
import { cn } from "@recast/ui/utils";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { EASE, type Easing, easingEquals } from "../../lib/easing/cubic-bezier";
import { clock, clockCentis as fmtTime } from "../../lib/format/time";
import { motionDuration } from "../../lib/motion.svelte";
import { registry } from "../../lib/registry";
import { resolveZoomCenter } from "../../lib/zoom/auto-apply";
import { overlappingZoomIds } from "../../lib/zoom/resolve";
import {
	DEFAULT_ZOOM_CENTER,
	type EditorStore,
	type ZoomRegion,
} from "../../stores/editor-store.svelte";
import EasingControl from "./EasingControl.svelte";
import FocusPad from "./FocusPad.svelte";
import {
	computeNewZoomBounds,
	isOutsideClip,
	regionMaxRamp,
	retimeEnd,
	retimeStart,
	sparklinePath,
} from "./focus-panel.logic";
import PanelSection from "./PanelSection.svelte";
import SliderRow from "./SliderRow.svelte";

interface Props {
	store: EditorStore;
	/** Regenerate auto-zoom regions from cursor activity. Owned by the editor page. */
	onRegenerateAutoZoom?: () => void;
}

let { store, onRegenerateAutoZoom }: Props = $props();

// Built-in + extension easing presets, from the registry.
const easingPresets = $derived(
	registry.list("easing").map((e) => ({ id: e.id, label: e.label, value: e.value.value })),
);

const selected = $derived<ZoomRegion | null>(
	store.zoomRegions.find((r) => r.id === store.selectedZoomRegionId) ?? null,
);

// Listed in timeline order (by start time) so the panel scans the same way the
// timeline reads, left to right, and numbered so a row correlates with the
// "Region N" detail header.
const orderedRegions = $derived([...store.zoomRegions].sort((a, b) => a.start - b.start));
const selectedIndex = $derived(
	selected ? orderedRegions.findIndex((r) => r.id === selected.id) : -1,
);

// NLE accessors, not raw trim fields: `outPoint` resolves the legacy
// `trimEnd === 0` sentinel, which the timeline lane already respects.
const clipIn = $derived(store.inPoint);
const clipOut = $derived(store.outPoint);

// Overlapping regions are ambiguous in preview and the FFmpeg export SUMS
// their zoom instead of picking one, so they get called out, not hidden.
const overlapping = $derived(new Set(overlappingZoomIds(store.zoomRegions)));
const outOfClip = (r: ZoomRegion) => isOutsideClip(r, clipIn, clipOut);

// Zoom is only legible with the playhead inside the region, so selecting one
// parks the playhead at the moment it reaches full scale.
function focusMoment(r: ZoomRegion) {
	const half = Math.max(0, (r.end - r.start) * 0.5);
	return Math.min(r.end - 0.01, r.start + Math.min(Math.max(0, r.rampIn), half) + 0.01);
}

function selectRegion(r: ZoomRegion) {
	store.selectedZoomRegionId = r.id;
	store.seek(focusMoment(r));
}

const playheadInSelected = $derived(
	selected ? store.currentTime > selected.start && store.currentTime < selected.end : true,
);

// Null when the playhead leaves no room, which disables the button instead of
// snapping the edge somewhere the user didn't point at.
const startFromPlayhead = $derived(
	selected ? retimeStart(selected, store.currentTime, clipIn) : null,
);
const endFromPlayhead = $derived(selected ? retimeEnd(selected, store.currentTime, clipOut) : null);

const frameAspect = $derived(
	store.metadata?.width && store.metadata?.height
		? store.metadata.width / store.metadata.height
		: 16 / 9,
);

// The preset both ramps share, or null once the curves diverge or go custom.
const activeEasingId = $derived.by(() => {
	if (!selected) return null;
	const r = selected;
	return (
		easingPresets.find((p) => easingEquals(r.easeIn, p.value) && easingEquals(r.easeOut, p.value))
			?.id ?? null
	);
});

function clampToClip(r: ZoomRegion) {
	store.pushUndoState();
	store.updateZoomRegion(r.id, {
		start: Math.max(clipIn, Math.min(r.start, clipOut - 0.1)),
		end: Math.min(clipOut, Math.max(r.end, clipIn + 0.1)),
	});
}

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
	onRegenerateAutoZoom?.();
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

// Curves only. It used to reset rampIn/rampOut too, which are Timing controls:
// a button in one section silently changing another section's values is the same
// trap `recenterFocus` is careful to avoid.
function resetCurves() {
	if (!selected) return;
	store.pushUndoState();
	store.updateZoomRegion(selected.id, {
		easeIn: { ...EASE },
		easeOut: { ...EASE },
	});
}

// The focus point only: scale and motion blur sit in the same section now, and
// Recenter must not quietly reset those too.
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

    <!-- A plain preference + a leading action, not a boxed banner: the box read
         as an alert. Toggle sets the persistent behaviour; the button runs it now. -->
    <div class="flex flex-col gap-2.5">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-[11px] font-medium text-foreground">Auto-zoom on import</p>
          <p class="text-[10px] leading-snug text-muted-foreground">
            Adds a focus moment at each click and pause in cursor movement.
          </p>
        </div>
        <SegmentedToggle
          checked={store.autoZoomEnabled}
          size="xs"
          aria-label="Auto-zoom on import"
          onCheckedChange={(next) => (store.autoZoomEnabled = next)}
        />
      </div>
      <div class="flex items-center gap-1.5">
        <Button
          variant="secondary"
          size="xs"
          class="gap-1.5"
          onclick={rerunAutoZoom}
          disabled={!store.cursorPath || !onRegenerateAutoZoom}
        >
          <AiWand size={11} />
          Generate now
        </Button>
        {#if hasAutoZooms}
          <Button
            variant="ghost"
            size="xs"
            class="text-muted-foreground hover:text-destructive"
            onclick={clearAuto}
          >
            Remove generated
          </Button>
        {/if}
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
                ? "border-foreground/40 bg-card shadow-(--shadow-craft-inset) ring-1 ring-inset ring-foreground/20"
                : "border-border/60 bg-card/60 hover:border-border hover:bg-card",
              isHidden && "opacity-55",
            )}
          >
            <button
              type="button"
              onclick={() => selectRegion(region)}
              aria-pressed={isActive}
              aria-label={`Zoom region ${i + 1}: ${region.scale.toFixed(1)}× at ${fmtTime(region.start)}`}
              class="absolute inset-0 z-0 rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
            ></button>
            <span
              class={cn(
                "pointer-events-none w-3.5 shrink-0 text-center text-[10px] font-semibold tabular-nums",
                isActive ? "text-foreground" : "text-muted-foreground/70",
              )}>{i + 1}</span>
            <span
              class={cn(
                "pointer-events-none flex h-8 w-12 shrink-0 items-center justify-center rounded-md border transition-colors",
                isActive
                  ? "border-foreground/40 bg-background/40 text-foreground"
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
              <div class="flex items-baseline gap-1.5">
                <span class="shrink-0 text-[12px] font-semibold tabular-nums text-foreground">
                  {region.scale.toFixed(2)}×
                </span>
                <span class="truncate text-[11px] tabular-nums text-muted-foreground">
                  {clock(region.start)}–{clock(region.end)}
                </span>
              </div>
              <div class="mt-0.5 flex items-center gap-1">
                <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                  {(region.end - region.start).toFixed(2)}s
                </span>
                {#if region.source === "auto"}
                  <span
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-border/60 bg-muted/60 px-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground"
                  >
                    <Sparkles size={8} />
                    Auto
                  </span>
                {/if}
                {#if isHidden}
                  <span
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-border bg-muted/60 px-1 text-[9px] font-medium text-muted-foreground"
                  >
                    <EyeOff size={8} />
                    Hidden
                  </span>
                {/if}
                {#if overlapping.has(region.id)}
                  <span
                    title="Overlaps another region. Only one can apply, and export and preview can disagree."
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-warning/40 bg-warning/10 px-1 text-[9px] font-medium text-warning"
                  >
                    <TriangleAlert size={8} />
                    Overlaps
                  </span>
                {/if}
                {#if outOfClip(region)}
                  <span
                    title="Part of this region sits outside the trimmed clip and will never play."
                    class="inline-flex shrink-0 items-center gap-0.5 rounded-sm border border-border bg-muted/60 px-1 text-[9px] font-medium text-muted-foreground"
                  >
                    Outside clip
                  </span>
                {/if}
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
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
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
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              >
                <Copy size={12} />
              </button>
              <button
                type="button"
                onclick={() => store.removeZoomRegion(region.id)}
                aria-label="Delete region"
                title="Delete"
                class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/15 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
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

      <!-- Both notices explain why the controls below may look like they do
           nothing, so they sit above the controls, not at the end of the panel. -->
      {#if !playheadInSelected}
        <button
          type="button"
          onclick={() => selectRegion(region)}
          class="flex items-center gap-2 rounded-lg border border-border/60 bg-card/60 px-2.5 py-1.5 text-left text-[10px] text-muted-foreground transition-colors hover:bg-card"
        >
          <Crosshair size={11} class="shrink-0" />
          <span class="flex-1">The playhead is outside this region, so its zoom isn't visible.</span>
          <span class="shrink-0 font-medium text-foreground">Jump to it</span>
        </button>
      {/if}

      {#if overlapping.has(region.id)}
        <div
          class="flex items-start gap-2 rounded-lg border border-warning/40 bg-warning/10 px-2.5 py-1.5 text-[10px] leading-snug text-warning"
        >
          <TriangleAlert size={11} class="mt-px shrink-0" />
          <span>
            Overlaps another region. Only one can apply at a time, and the exported
            video can differ from this preview. Trim one so they don't share time.
          </span>
        </div>
      {/if}

      {#if outOfClip(region)}
        <div
          class="flex items-center gap-2 rounded-lg border border-border/60 bg-card/60 px-2.5 py-1.5 text-[10px] text-muted-foreground"
        >
          <span class="flex-1 leading-snug">
            Part of this region sits outside the trimmed clip and will never play.
          </span>
          <Button variant="outline" size="xs" onclick={() => clampToClip(region)}>
            Fit to clip
          </Button>
        </div>
      {/if}

      <PanelSection
        title="Zoom"
        hint="How far in, and where. Drag the pad to move the focus point; the outlined box is what the viewer sees."
      >
        {#snippet action()}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5"
            onclick={recenterFocus}
            disabled={region.centerX === DEFAULT_ZOOM_CENTER &&
              region.centerY === DEFAULT_ZOOM_CENTER}
          >
            <Crosshair size={11} />
            Recenter
          </Button>
        {/snippet}

        <FocusPad
          centerX={region.centerX}
          centerY={region.centerY}
          scale={region.scale}
          aspect={frameAspect}
          onstart={() => store.pushUndoState()}
          onchange={(x, y) => updateSelected({ centerX: x, centerY: y })}
        />

        <SliderRow
          label="Scale"
          value={region.scale}
          min={1}
          max={3}
          step={0.05}
          formatValue={(v) => `${v.toFixed(2)}×`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ scale: v })}
        />
        <SliderRow
          label="Focus X"
          value={region.centerX}
          min={0}
          max={1}
          step={0.01}
          formatValue={(v) => v.toFixed(2)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ centerX: v })}
        />
        <SliderRow
          label="Focus Y"
          value={region.centerY}
          min={0}
          max={1}
          step={0.01}
          formatValue={(v) => v.toFixed(2)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ centerY: v })}
        />

        <!-- Preview-only, and it must say so: the Rust compositor has no zoom
             motion blur (only the cursor trail does), and every region defaults
             to 0.5, so exports come out sharper than the editor looks. -->
        <SliderRow
          label="Blur"
          description="Preview only. Not applied to exported video."
          value={Math.round(region.motionBlur * 100)}
          min={0}
          max={100}
          step={1}
          formatValue={(v) => (v === 0 ? "Off" : `${v.toFixed(0)}%`)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ motionBlur: v / 100 })}
        />
        {#if region.motionBlur > 0.001}
          <p class="px-0.5 text-[10px] leading-snug text-muted-foreground">
            Motion blur shows in the preview only. The exported video is not blurred.
          </p>
        {/if}
      </PanelSection>

      <PanelSection
        title="Timing"
        hint="When the region runs and how long it ramps in and out. Use split ramps to hold at full zoom before releasing."
      >
        {#snippet action()}
          <!-- The playhead is how every other edit is timed, so it should be
               able to set these two without dragging a slider to a timecode. -->
          <div class="flex items-center gap-1">
            <Button
              variant="ghost"
              size="xs"
              class="text-[10px]"
              disabled={!startFromPlayhead}
              title="Move the region's start to the playhead"
              onclick={() => startFromPlayhead && updateSelected(startFromPlayhead, true)}
            >
              Start here
            </Button>
            <Button
              variant="ghost"
              size="xs"
              class="text-[10px]"
              disabled={!endFromPlayhead}
              title="Move the region's end to the playhead"
              onclick={() => endFromPlayhead && updateSelected(endFromPlayhead, true)}
            >
              End here
            </Button>
          </div>
        {/snippet}
        <SliderRow
          label="Start"
          value={region.start}
          min={clipIn}
          max={Math.max(region.end - 0.1, clipIn)}
          step={0.01}
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ start: v })}
        />
        <SliderRow
          label="End"
          value={region.end}
          min={region.start + 0.1}
          max={clipOut}
          step={0.01}
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ end: v })}
        />
        <SliderRow
          label="Ramp in"
          value={region.rampIn}
          min={0}
          max={Math.max(maxRamp, 0.01)}
          step={0.01}
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampIn: v })}
        />
        <SliderRow
          label="Ramp out"
          value={region.rampOut}
          min={0}
          max={Math.max(maxRamp, 0.01)}
          step={0.01}
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampOut: v })}
        />
      </PanelSection>

      <!-- Presets lead; raw bezier curves live behind a "Custom curves" disclosure. -->
      <PanelSection
        title="Easing"
        hint="How the zoom accelerates in and decelerates out."
        collapsible
        defaultOpen={false}
      >
        {#snippet action()}
          {#if !activeEasingId}
            <span class="text-[11px] text-muted-foreground">Custom</span>
          {/if}
        {/snippet}
        <EasingControl
          value={{ in: region.easeIn, out: region.easeOut }}
          onpick={applyPresetToBoth}
          ondrag={(next, which) => {
            // Fires per pointermove, so coalesce instead of one entry per frame.
            store.pushUndoStateCoalesced(`zoom-curve-${region.id}-${which}`, 500);
            updateSelected(which === "out" ? { easeOut: next } : { easeIn: next });
          }}
          size={220}
        />
        <div class="flex justify-end">
          <Button variant="ghost" size="xs" class="text-[10px]" onclick={resetCurves}>
            Reset curves
          </Button>
        </div>
      </PanelSection>
    </div>
  {/if}
</div>
