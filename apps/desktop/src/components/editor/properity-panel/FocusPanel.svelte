<script lang="ts">
  import { EASE, EASING_PRESETS, type Easing } from "$lib/easing/cubic-bezier";
  import {
    DEFAULT_ZOOM_CENTER,
    DEFAULT_ZOOM_MOTION_BLUR,
    DEFAULT_ZOOM_RAMP,
    type EditorStore,
    type ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import { resolveZoomCenter } from "$lib/zoom/auto-apply";
  import { Crosshair, Plus, Sparkles, Target, Trash2, Wand2 } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import BezierEditor from "../_components/BezierEditor.svelte";
  import { SliderControl } from "@recast/ui/slider-control";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  const selected = $derived<ZoomRegion | null>(
    store.zoomRegions.find((r) => r.id === store.selectedZoomRegionId) ?? null
  );

  function addRegion() {
    const duration = store.metadata?.duration ?? 0;
    if (duration <= 0) return;
    const clipEnd = store.trimEnd || duration;
    const start = Math.max(store.trimStart, store.currentTime - 0.35);
    const end = Math.min(clipEnd, Math.max(start + 0.8, store.currentTime + 0.85));
    // Look up where the cursor actually was at this moment so the new
    // region zooms toward the user's pointer rather than dead-centre.
    const w = store.metadata?.width ?? 0;
    const h = store.metadata?.height ?? 0;
    const center = resolveZoomCenter(store.cursorSamplesRaw, store.currentTime, w, h);
    store.addZoomRegion(start, end, 1.8, center);
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

  function recenterFocus() {
    if (!selected) return;
    store.pushUndoState();
    store.updateZoomRegion(selected.id, {
      centerX: DEFAULT_ZOOM_CENTER,
      centerY: DEFAULT_ZOOM_CENTER,
      motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
    });
  }

  function fmtTime(sec: number): string {
    const total = Math.max(0, sec);
    const s = Math.floor(total);
    const ms = Math.round((total - s) * 1000);
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}.${ms
      .toString()
      .padStart(3, "0")
      .slice(0, 2)}`;
  }

  function regionMaxRamp(r: ZoomRegion): number {
    return Math.max(0, (r.end - r.start) * 0.5);
  }

  // Precompute the sparkline path for one region card — encodes the
  // rampIn → hold → rampOut shape as a normalised 1.0 → scale → 1.0 curve.
  function sparklinePath(r: ZoomRegion, w: number, h: number): string {
    const duration = Math.max(0.001, r.end - r.start);
    const maxScale = Math.max(r.scale, 1.0);
    const normScale = (s: number) =>
      maxScale === 1 ? 1 : (s - 1) / (maxScale - 1);
    const samples: Array<[number, number]> = [];
    const N = 40;
    for (let i = 0; i <= N; i++) {
      const t = (i / N) * duration;
      const absT = r.start + t;
      const s = scaleAt(r, absT);
      const x = (t / duration) * w;
      const y = h - normScale(s) * h * 0.9 - 1;
      samples.push([x, y]);
    }
    return samples.map(([x, y], i) => `${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`).join(" ");
  }

  function scaleAt(r: ZoomRegion, t: number): number {
    if (t <= r.start || t >= r.end) return 1;
    const duration = Math.max(0, r.end - r.start);
    const half = duration * 0.5;
    const rampIn = Math.min(Math.max(0, r.rampIn), half);
    const rampOut = Math.min(Math.max(0, r.rampOut), half);
    const holdStart = r.start + rampIn;
    const holdEnd = r.end - rampOut;
    let phase: number, curve: Easing;
    if (t < holdStart) {
      phase = rampIn > 0 ? (t - r.start) / rampIn : 1;
      curve = r.easeIn;
    } else if (t > holdEnd) {
      phase = rampOut > 0 ? (r.end - t) / rampOut : 1;
      curve = r.easeOut;
    } else {
      return r.scale;
    }
    phase = Math.max(0, Math.min(1, phase));
    // Low-budget x→y approximation (polynomial-in-t with t ≈ x). Indistinguishable
    // at sparkline resolution; avoids pulling in the full Newton-Raphson solver.
    const a = 1 - 3 * curve.y2 + 3 * curve.y1;
    const b = 3 * curve.y2 - 6 * curve.y1;
    const c = 3 * curve.y1;
    const s = ((a * phase + b) * phase + c) * phase;
    return 1 + (r.scale - 1) * s;
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
    hint="Each region zooms in on the clip with your own ease-in and ease-out curves. Use split ramps to hold at full zoom before releasing."
    flush
  >
    {#snippet action()}
      <Button variant="secondary" size="xs" class="gap-1.5" onclick={addRegion}>
        <Plus size={11} />
        Add
      </Button>
    {/snippet}
    <!-- Smart Auto-Zoom controls -->
    <div class="flex flex-col gap-1.5 rounded-md border border-border/60 bg-card/40 p-2">
      <label class="flex items-center gap-2 text-[11px] text-foreground">
        <input
          type="checkbox"
          class="size-3 accent-primary"
          checked={store.autoZoomEnabled}
          onchange={(e) => (store.autoZoomEnabled = (e.target as HTMLInputElement).checked)}
        />
        <Sparkles size={11} class="text-primary" />
        <span class="flex-1">Smart Auto-Zoom on import</span>
      </label>
      <p class="text-[10px] leading-snug text-muted-foreground">
        Adds a focus moment at every click and settle point when a recording first opens.
      </p>
      <div class="flex items-center justify-end gap-1 pt-0.5">
        {#if hasAutoZooms}
          <Button variant="ghost" size="xs" onclick={clearAuto}>Clear auto zooms</Button>
        {/if}
        <Button
          variant="secondary"
          size="xs"
          class="gap-1.5"
          onclick={rerunAutoZoom}
          disabled={!store.cursorPath}
        >
          <Wand2 size={11} />
          Re-run
        </Button>
      </div>
    </div>
  </PanelSection>

  <!-- Region list -->
  {#if store.zoomRegions.length === 0}
    <div
      class="flex flex-col items-center gap-2 rounded-md border border-dashed border-border bg-card/40 px-3 py-6 text-center"
    >
      <Target size={18} class="text-muted-foreground" />
      <p class="text-[11px] font-medium text-foreground">No focus regions yet</p>
      <p class="text-[10px] text-muted-foreground">
        Park the playhead where you want to zoom, then press Add.
      </p>
    </div>
  {:else}
    <section class="flex flex-col gap-1">
      {#each store.zoomRegions as region (region.id)}
        {@const isActive = region.id === store.selectedZoomRegionId}
        <button
          type="button"
          onclick={() => (store.selectedZoomRegionId = region.id)}
          class={cn(
            "group flex items-center gap-2 rounded-md border px-2 py-1.5 text-left transition-colors",
            "focus:outline-none focus:ring-1 focus:ring-ring",
            isActive
              ? "border-primary bg-primary/10"
              : "border-border bg-card hover:bg-muted/50"
          )}
        >
          <div class="flex-1 min-w-0 flex items-center gap-2">
            <svg viewBox="0 0 100 18" width="48" height="14" class="shrink-0 text-primary">
              <path
                d={sparklinePath(region, 100, 18)}
                stroke="currentColor"
                stroke-width="1.4"
                fill="none"
              />
            </svg>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5 truncate">
                <span class="truncate text-[11px] font-medium text-foreground">
                  {region.scale.toFixed(2)}× · {fmtTime(region.start)}–{fmtTime(region.end)}
                </span>
                {#if region.source === "auto"}
                  <span
                    class="shrink-0 rounded-sm border border-primary/30 bg-primary/10 px-1 text-[9px] font-semibold uppercase tracking-wider text-primary"
                  >
                    Auto
                  </span>
                {/if}
              </div>
              <div class="text-[10px] text-muted-foreground">
                {(region.end - region.start).toFixed(2)}s
              </div>
            </div>
          </div>
        </button>
      {/each}
    </section>
  {/if}

  <!-- Region editor -->
  {#if selected}
    {@const region = selected}
    {@const maxRamp = regionMaxRamp(region)}
    <div class="flex flex-col gap-3 border-t border-border pt-3">
      <PanelSection title="Settings">
        {#snippet action()}
          <Button
            variant="destructive_soft"
            size="xs"
            class="gap-1.5"
            onclick={removeSelected}
          >
            <Trash2 size={11} />
            Delete
          </Button>
        {/snippet}
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
        />
      </PanelSection>

      <PanelSection
        title="Focus point"
        hint="Drag the rectangle on the preview, or use the sliders. Values are 0..1 across the frame (0.5 = center)."
      >
        {#snippet action()}
          <Button
            variant="ghost"
            size="xs"
            onclick={recenterFocus}
            disabled={region.centerX === 0.5 &&
              region.centerY === 0.5 &&
              region.motionBlur === DEFAULT_ZOOM_MOTION_BLUR}
          >
            <Crosshair size={11} class="mr-1" />
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
        />
        <SliderControl
          label="Focus Y"
          value={region.centerY}
          min={0}
          max={1}
          step={0.01}
          formatValue={(v) => v.toFixed(2)}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ centerY: v })}
        />
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
        />
      </PanelSection>

      <PanelSection title="Timing">
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
        />
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
        />
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
        />
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
        />
      </PanelSection>

      <PanelSection title="Easing" flush>
        {#snippet action()}
          <Button variant="ghost" size="xs" onclick={resetCurves}>Reset</Button>
        {/snippet}
        <div class="flex flex-col gap-2">
          <div class="grid grid-cols-2 gap-3">
            <BezierEditor
              label="Ease in"
              value={region.easeIn}
              onchange={(v) => updateSelected({ easeIn: v }, true)}
              showPresets={false}
              size={140}
            />
            <BezierEditor
              label="Ease out"
              value={region.easeOut}
              onchange={(v) => updateSelected({ easeOut: v }, true)}
              showPresets={false}
              size={140}
            />
          </div>
          <div class="flex flex-wrap gap-1">
            {#each EASING_PRESETS as preset (preset.id)}
              <button
                type="button"
                onclick={() => applyPresetToBoth(preset.value)}
                class="h-6 rounded-sm border border-border bg-background px-2 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              >
                {preset.label}
              </button>
            {/each}
          </div>
        </div>
      </PanelSection>
    </div>
  {/if}
</div>
