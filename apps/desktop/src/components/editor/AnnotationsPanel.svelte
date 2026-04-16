<script lang="ts">
  import { EASE } from "$lib/easing/cubic-bezier";
  import {
    DEFAULT_ANNOTATION_RAMP,
    type Annotation,
    type AnnotationKindName,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { Circle, Pencil, Plus, Square, Trash2 } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import BezierEditor from "./BezierEditor.svelte";
  import InspectorHint from "./InspectorHint.svelte";
  import SliderControl from "./SliderControl.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  const selected = $derived<Annotation | null>(
    store.annotations.find((a) => a.id === store.selectedAnnotationId) ?? null,
  );

  const tools: { id: AnnotationKindName; label: string; icon: typeof Square }[] = [
    { id: "rect", label: "Rectangle", icon: Square },
    { id: "ellipse", label: "Ellipse", icon: Circle },
  ];

  function toggleTool(id: AnnotationKindName) {
    store.annotationTool = store.annotationTool === id ? null : id;
  }

  function fmtTime(sec: number): string {
    const s = Math.max(0, sec);
    const m = Math.floor(s / 60);
    const rem = s - m * 60;
    return `${m}:${rem.toFixed(2).padStart(5, "0")}`;
  }

  function updateSelected(updates: Partial<Annotation>, trackUndo = false) {
    if (!selected) return;
    if (trackUndo) store.pushUndoState();
    store.updateAnnotation(selected.id, updates);
  }

  function setStroke(update: Partial<Annotation["stroke"]>) {
    if (!selected) return;
    store.updateAnnotation(selected.id, {
      stroke: { ...selected.stroke, ...update },
    });
  }

  function resetCurves() {
    if (!selected) return;
    store.pushUndoState();
    store.updateAnnotation(selected.id, {
      easeIn: { ...EASE },
      easeOut: { ...EASE },
      rampIn: DEFAULT_ANNOTATION_RAMP,
      rampOut: DEFAULT_ANNOTATION_RAMP,
    });
  }

  function kindIcon(a: Annotation): typeof Square {
    return a.kind.kind === "rect" ? Square : Circle;
  }

  const STROKE_SWATCHES = [
    "#3b82f6",
    "#ef4444",
    "#22c55e",
    "#f59e0b",
    "#a855f7",
    "#ec4899",
    "#06b6d4",
    "#ffffff",
  ];
  const FILL_SWATCHES = [
    "transparent",
    "rgba(59,130,246,0.20)",
    "rgba(239,68,68,0.20)",
    "rgba(34,197,94,0.20)",
    "rgba(245,158,11,0.20)",
    "rgba(168,85,247,0.20)",
    "rgba(0,0,0,0.35)",
    "rgba(255,255,255,0.20)",
  ];

  function maxRamp(a: Annotation): number {
    return Math.max(0, (a.end - a.start) * 0.5);
  }
</script>

<div class="flex flex-col gap-5 animate-in fade-in duration-200">
  <!-- Tool palette -->
  <section>
    <div class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-1.5">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Annotations
        </h3>
        <InspectorHint content="Draw overlays on the video. Pick a tool, then drag on the preview. Each annotation is anchored in video-space so it follows zoom and crop." />
      </div>
      <Pencil size={11} class="text-muted-foreground" />
    </div>
    <div class="mt-2 flex flex-wrap gap-1">
      {#each tools as tool (tool.id)}
        {@const Icon = tool.icon}
        {@const isActive = store.annotationTool === tool.id}
        <button
          type="button"
          aria-pressed={isActive}
          onclick={() => toggleTool(tool.id)}
          class={cn(
            "flex h-7 items-center gap-1.5 rounded-md border px-2 text-[11px] font-medium transition-colors",
            "focus:outline-none focus:ring-1 focus:ring-ring",
            isActive
              ? "border-primary bg-primary/10 text-primary"
              : "border-border bg-background text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon size={12} />
          {tool.label}
        </button>
      {/each}
    </div>
    {#if store.annotationTool}
      <p class="mt-1.5 text-[10px] text-muted-foreground">
        Drag on the preview to place. <kbd class="rounded border border-border bg-background px-1 font-mono text-[9px]">Esc</kbd> to cancel.
      </p>
    {/if}
  </section>

  <!-- Annotation list -->
  {#if store.annotations.length === 0}
    <div
      class="flex flex-col items-center gap-2 rounded-md border border-dashed border-border bg-card/40 px-3 py-6 text-center"
    >
      <Pencil size={18} class="text-muted-foreground" />
      <p class="text-[11px] font-medium text-foreground">No annotations yet</p>
      <p class="text-[10px] text-muted-foreground">
        Pick a tool above, then drag on the preview.
      </p>
    </div>
  {:else}
    <section class="flex flex-col gap-1">
      {#each store.annotations as annotation (annotation.id)}
        {@const isActive = annotation.id === store.selectedAnnotationId}
        {@const Icon = kindIcon(annotation)}
        <button
          type="button"
          onclick={() => (store.selectedAnnotationId = annotation.id)}
          class={cn(
            "flex items-center gap-2 rounded-md border px-2 py-1.5 text-left transition-colors",
            "focus:outline-none focus:ring-1 focus:ring-ring",
            isActive
              ? "border-primary bg-primary/10"
              : "border-border bg-card hover:bg-muted/50",
          )}
        >
          <Icon size={12} class="shrink-0 text-primary" />
          <div class="flex-1 min-w-0">
            <div class="truncate text-[11px] font-medium text-foreground">
              {annotation.kind.kind === "rect" ? "Rectangle" : "Ellipse"}
            </div>
            <div class="text-[10px] text-muted-foreground">
              {fmtTime(annotation.start)}–{fmtTime(annotation.end)}
            </div>
          </div>
        </button>
      {/each}
    </section>
  {/if}

  <!-- Selected annotation editor -->
  {#if selected}
    {@const a = selected}
    <section class="flex flex-col gap-3 border-t border-border pt-3">
      <header class="flex items-center justify-between gap-2">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Selected
        </h3>
        <Button
          variant="destructive_soft"
          size="xs"
          class="gap-1.5"
          onclick={() => store.removeAnnotation(a.id)}
        >
          <Trash2 size={11} />
          Delete
        </Button>
      </header>

      <!-- Timing -->
      <div class="space-y-2.5">
        <SliderControl
          label="Start"
          value={a.start}
          min={0}
          max={Math.max(a.end - 0.1, 0)}
          step={0.05}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ start: v })}
        />
        <SliderControl
          label="End"
          value={a.end}
          min={a.start + 0.1}
          max={store.metadata?.duration ?? a.end}
          step={0.05}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ end: v })}
        />
        <SliderControl
          label="Fade in"
          value={a.rampIn}
          min={0}
          max={Math.max(maxRamp(a), 0.01)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampIn: v })}
        />
        <SliderControl
          label="Fade out"
          value={a.rampOut}
          min={0}
          max={Math.max(maxRamp(a), 0.01)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampOut: v })}
        />
      </div>

      <!-- Fade curves -->
      <div class="grid grid-cols-2 gap-3">
        <BezierEditor
          label="Fade in"
          value={a.easeIn}
          onchange={(v) => updateSelected({ easeIn: v }, true)}
          showPresets={false}
          size={130}
        />
        <BezierEditor
          label="Fade out"
          value={a.easeOut}
          onchange={(v) => updateSelected({ easeOut: v }, true)}
          showPresets={false}
          size={130}
        />
      </div>
      <div class="flex justify-end">
        <Button variant="ghost" size="xs" onclick={resetCurves}>Reset curves</Button>
      </div>

      <!-- Appearance -->
      <div class="space-y-2.5 border-t border-border pt-3">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Stroke
        </h3>
        <SliderControl
          label="Width"
          value={a.stroke.width * 1000}
          min={0}
          max={20}
          step={1}
          unit="‰"
          description="Per-mille of video width; 0 disables the stroke."
          formatValue={(v) => `${v.toFixed(0)}‰`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => setStroke({ width: v / 1000 })}
        />
        <div class="flex flex-wrap gap-1">
          {#each STROKE_SWATCHES as swatch (swatch)}
            {@const isActive = a.stroke.color === swatch}
            <button
              type="button"
              aria-label="Stroke {swatch}"
              aria-pressed={isActive}
              onclick={() => setStroke({ color: swatch })}
              class={cn(
                "size-5 rounded-full border-2 transition",
                isActive ? "border-ring ring-1 ring-ring" : "border-border",
              )}
              style:background={swatch}
            ></button>
          {/each}
        </div>
      </div>

      <div class="space-y-2.5 border-t border-border pt-3">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Fill
        </h3>
        <div class="flex flex-wrap gap-1">
          {#each FILL_SWATCHES as swatch (swatch)}
            {@const isActive = a.fill === swatch}
            <button
              type="button"
              aria-label={swatch === "transparent" ? "No fill" : `Fill ${swatch}`}
              aria-pressed={isActive}
              onclick={() => updateSelected({ fill: swatch })}
              class={cn(
                "size-5 rounded-md border-2 transition",
                isActive ? "border-ring ring-1 ring-ring" : "border-border",
                swatch === "transparent" && "bg-background",
              )}
              style:background={swatch === "transparent" ? undefined : swatch}
            >
              {#if swatch === "transparent"}
                <span class="block h-full w-full rounded-sm" style="background: repeating-linear-gradient(45deg, var(--color-muted) 0 3px, transparent 3px 6px);"></span>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      {#if a.kind.kind === "rect"}
        <SliderControl
          label="Corner radius"
          value={a.kind.radius * 1000}
          min={0}
          max={50}
          step={1}
          unit="‰"
          formatValue={(v) => `${v.toFixed(0)}‰`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => {
            if (a.kind.kind !== "rect") return;
            updateSelected({
              kind: { ...a.kind, radius: v / 1000 },
            });
          }}
        />
      {/if}
    </section>
  {:else}
    <section class="rounded-md border border-border bg-card/40 px-3 py-3 text-center text-[10px] text-muted-foreground">
      Select an annotation to edit its timing, curves, and appearance.
    </section>
  {/if}

  <!-- Phase 2/3 teaser so the missing tools don't look like a bug -->
  <section class="rounded-md border border-dashed border-border bg-card/30 px-3 py-2.5">
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
      Coming next
    </p>
    <p class="mt-1 text-[10px] text-muted-foreground">
      Arrows, polygons, blur redactions, images, and text (with Google-Fonts picker) land in the next pass.
    </p>
  </section>
</div>
