<script lang="ts">
  import { kindIcon, kindLabel } from "$lib/annotations/kind-label";
  import { isEditableTarget } from "$lib/dom/editable";
  import { clockCentis as fmtTime } from "$lib/format/time";
  import { imageFileName, toolHint as toolHintFor } from "./annotations-panel.logic";
  import { regionMaxRamp } from "./focus-panel.logic";
  import { FONT_WEIGHTS, STROKE_SWATCHES } from "$lib/annotations/palette";
  import {
    getRecentColors,
    pushRecentColor,
  } from "$lib/annotations/recent-colors";
  import { EASE } from "$lib/easing/cubic-bezier";
  import {
    DEFAULT_ANNOTATION_RAMP,
    type Annotation,
    type AnnotationKindName,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    AlignCenter,
    AlignLeft,
    AlignRight,
    ArrowUpRight,
    Circle,
    Droplets,
    Image as ImageIcon,
    MousePointer2,
    Square,
    SquareDashedMousePointer,
    Trash2,
    Type as TypeIcon,
  } from "@lucide/svelte";
  import { toast } from "@recast/ui/sonner";
  import { pickImageAnnotation, pickImageFile } from "$lib/annotations/image-import";
  import { Button } from "@recast/ui/button";
  import { ColorField } from "@recast/ui/color-field";
  import { Kbd } from "@recast/ui/kbd";
  import { Segmented } from "@recast/ui/segmented";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import FontPicker from "./FontPicker.svelte";
  import { SliderControl } from "@recast/ui/slider-control";
  import { Textarea } from "@recast/ui/textarea";
  import { cn } from "@recast/ui/utils";
  import BezierEditor from "../_components/BezierEditor.svelte";
  import AnnotationAppearance from "./annotations/AnnotationAppearance.svelte";
  import AnnotationGeometry from "./annotations/AnnotationGeometry.svelte";
  import AnnotationLayerPanel from "./annotations/AnnotationLayerPanel.svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  let recents = $state<string[]>(getRecentColors());
  function rememberColor(color: string) {
    recents = pushRecentColor(color);
  }

  const selected = $derived<Annotation | null>(
    store.annotations.find((a) => a.id === store.selectedAnnotationId) ?? null,
  );

  // Which ramp the Fade-curves editor targets (one graph at a time).
  let customCurve = $state<"in" | "out">("in");

  type ToolDef = {
    id: AnnotationKindName | "select";
    label: string;
    icon: typeof Square;
    hotkey: string;
  };

  // Working tools only. Disabled/locked roadmap tiles are clutter.
  const tools: ToolDef[] = [
    { id: "select", label: "Select", icon: MousePointer2, hotkey: "V" },
    { id: "rect", label: "Rectangle", icon: Square, hotkey: "R" },
    { id: "ellipse", label: "Ellipse", icon: Circle, hotkey: "O" },
    { id: "arrow", label: "Arrow", icon: ArrowUpRight, hotkey: "A" },
    { id: "text", label: "Text", icon: TypeIcon, hotkey: "T" },
    { id: "image", label: "Image", icon: ImageIcon, hotkey: "I" },
    { id: "blur", label: "Blur", icon: Droplets, hotkey: "B" },
  ];

  function setTool(id: ToolDef["id"]) {
    if (id === "select") {
      store.annotationTool = null;
      return;
    }
    // Image is an insert action, not a draggable tool: pick a file, then place
    // it centered at its own aspect ratio.
    if (id === "image") {
      void insertImage();
      return;
    }
    store.annotationTool = store.annotationTool === id ? null : id;
  }

  async function insertImage() {
    store.annotationTool = null;
    const meta = store.metadata;
    const frameAspect = meta && meta.height > 0 ? meta.width / meta.height : 16 / 9;
    try {
      const kind = await pickImageAnnotation(frameAspect);
      if (kind) store.addAnnotation(kind);
    } catch (error) {
      toast.error(`Could not insert image: ${error}`);
    }
  }

  async function replaceImage() {
    if (!selected || selected.kind.kind !== "image") return;
    try {
      const path = await pickImageFile();
      if (!path) return;
      store.pushUndoState();
      store.updateAnnotation(selected.id, {
        kind: { ...selected.kind, path },
      });
    } catch (error) {
      toast.error(`Could not replace image: ${error}`);
    }
  }

  // Tool hotkeys. Suppressed when focus is in an editable element so typing
  // in a text annotation or any input doesn't switch tools.
  function handleHotkey(event: KeyboardEvent) {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (isEditableTarget(event.target)) return;
    const key = event.key.toLowerCase();
    const tool = tools.find((t) => t.hotkey.toLowerCase() === key);
    if (!tool) return;
    event.preventDefault();
    setTool(tool.id);
  }



  function updateSelected(updates: Partial<Annotation>, trackUndo = false) {
    if (!selected) return;
    if (trackUndo) store.pushUndoState();
    store.updateAnnotation(selected.id, updates);
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

  const toolHint = $derived(toolHintFor(store.annotationTool));
</script>

<!-- Tool hotkeys (V/R/O/A/T/B). `<svelte:window>` so HMR can't leak the listener. -->
<svelte:window onkeydown={handleHotkey} />

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <PanelSection
    title="Tools"
    hint="Pick a tool, then drag on the preview. Annotations follow zoom and crop. Esc cancels placement; hold Alt to bypass snap."
    flush
  >
    {#snippet action()}
      <div class="flex items-center gap-1.5">
        <span class="text-[10px] text-muted-foreground">Snap</span>
        <SegmentedToggle
          checked={store.annotationSnapEnabled}
          size="xs"
          aria-label="Snap to guides"
          onCheckedChange={(next) => (store.annotationSnapEnabled = next)}
        />
      </div>
    {/snippet}

    <div class="grid grid-cols-3 gap-1">
      {#each tools as tool (tool.id)}
        {@const Icon = tool.icon}
        {@const isActive =
          tool.id === "select"
            ? store.annotationTool === null
            : store.annotationTool === tool.id}
        <button
          type="button"
          aria-pressed={isActive}
          onclick={() => setTool(tool.id)}
          title={`${tool.label} (${tool.hotkey})`}
          class={cn(
            "group flex h-12 flex-col items-center justify-center gap-1 rounded-md border text-[10px] font-medium transition-all duration-150",
            "focus:outline-none focus:ring-2 focus:ring-ring/40",
            isActive
              ? "border-primary/60 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
              : "border-border/60 bg-card/60 text-muted-foreground hover:border-border hover:text-foreground",
          )}
        >
          <Icon size={14} />
          <span class="leading-none">{tool.label}</span>
        </button>
      {/each}
    </div>
    {#if toolHint}
      <p class="mt-1.5 text-[10px] text-muted-foreground">
        {toolHint}
        <Kbd class="ml-1">Esc</Kbd>
        to cancel.
      </p>
    {/if}
  </PanelSection>

  {#if store.annotations.length === 0}
    <div
      class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border/70 bg-card/40 px-3 py-6 text-center"
    >
      <div
        class="flex size-9 items-center justify-center rounded-lg border border-border/60 bg-card/70 text-muted-foreground shadow-(--shadow-craft-inset)"
      >
        <SquareDashedMousePointer size={16} />
      </div>
      <p class="text-[11px] font-medium text-foreground">No annotations yet</p>
      <p class="text-[10px] leading-snug text-muted-foreground">
        Pick a tool above, then drag on the preview.
      </p>
    </div>
  {:else}
    <AnnotationLayerPanel {store} />
  {/if}

  <!-- Selected annotation editor: appearance/content lead; timing, fade
       curves, geometry collapse below. -->
  {#if selected}
    {@const a = selected}
    {@const Icon = kindIcon(a)}
    <div class="flex flex-col gap-3 border-t border-border/50 pt-3">
      <div class="flex items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-1.5">
          <span
            class="grid size-5 shrink-0 place-items-center rounded bg-primary/15 text-primary"
          >
            <Icon size={11} />
          </span>
          <div class="min-w-0">
            <p
              class="truncate text-[11px] font-semibold tracking-tight text-foreground"
            >
              {kindLabel(a)}
            </p>
            <p class="text-[10px] tabular-nums text-muted-foreground">
              {fmtTime(a.start)}–{fmtTime(a.end)}
            </p>
          </div>
        </div>
        <Button
          variant="destructive_soft"
          size="xs"
          class="shrink-0 gap-1.5"
          onclick={() => store.removeAnnotation(a.id)}
        >
          <Trash2 size={11} />
          Delete
        </Button>
      </div>

      <PanelSection
        title="Anchor"
        hint="Video moves with zoom/focus; Frame pins it to the output frame."
      >
        <Segmented
          size="xs"
          aria-label="Anchor"
          value={a.anchor ?? "video"}
          options={[
            { value: "video", label: "Video" },
            { value: "frame", label: "Frame" },
          ]}
          onValueChange={(v) => {
            store.pushUndoState();
            updateSelected({ anchor: v as "video" | "frame" });
          }}
        />
      </PanelSection>

      {#if a.kind.kind === "text"}
        {@const k = a.kind}
        <PanelSection title="Text">
          <div class="flex flex-col gap-1">
            <span class="text-[10px] text-muted-foreground">Content</span>
            <Textarea
              rows={2}
              value={k.content}
              onfocus={() => store.pushUndoState()}
              oninput={(e) => {
                if (a.kind.kind !== "text") return;
                updateSelected({
                  kind: {
                    ...a.kind,
                    content: (e.currentTarget as HTMLTextAreaElement).value,
                  },
                });
              }}
              class="min-h-14 resize-none text-[11px]"
            />
          </div>

          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Font</span>
            <FontPicker
              value={k.fontFamily}
              weight={k.fontWeight}
              onChange={(v) => {
                if (a.kind.kind !== "text") return;
                store.pushUndoState();
                updateSelected({ kind: { ...a.kind, fontFamily: v } });
              }}
            />
          </div>

          <SliderControl
            label="Size"
            value={k.fontSize * 100}
            min={2}
            max={20}
            step={0.5}
            unit="%"
            description="Percentage of canvas height."
            formatValue={(v) => `${v.toFixed(1)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "text") return;
              updateSelected({ kind: { ...a.kind, fontSize: v / 100 } });
            }}
          />

          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Weight</span>
            <Segmented
              size="xs"
              fill={false}
              aria-label="Font weight"
              value={String(k.fontWeight)}
              options={FONT_WEIGHTS.map((w) => ({
                value: String(w.value),
                label: w.label,
                title: w.title,
              }))}
              onValueChange={(v) => {
                if (a.kind.kind !== "text") return;
                store.pushUndoState();
                updateSelected({
                  kind: {
                    ...a.kind,
                    fontWeight: Number(v) as 400 | 500 | 600 | 700,
                  },
                });
              }}
            />
          </div>

          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Align</span>
            {#snippet alignLeftIcon()}<AlignLeft size={12} />{/snippet}
            {#snippet alignCenterIcon()}<AlignCenter size={12} />{/snippet}
            {#snippet alignRightIcon()}<AlignRight size={12} />{/snippet}
            <Segmented
              size="xs"
              fill={false}
              aria-label="Text alignment"
              value={k.align}
              options={[
                { value: "left", icon: alignLeftIcon, title: "Left" },
                { value: "center", icon: alignCenterIcon, title: "Center" },
                { value: "right", icon: alignRightIcon, title: "Right" },
              ]}
              onValueChange={(v) => {
                if (a.kind.kind !== "text") return;
                store.pushUndoState();
                updateSelected({
                  kind: {
                    ...a.kind,
                    align: v as "left" | "center" | "right",
                  },
                });
              }}
            />
          </div>

          <ColorField
            label="Color"
            value={k.color}
            swatches={STROKE_SWATCHES}
            {recents}
            oncommit={(c: string) => {
              if (a.kind.kind !== "text") return;
              store.pushUndoState();
              updateSelected({ kind: { ...a.kind, color: c } });
              rememberColor(c);
            }}
          />
        </PanelSection>
      {/if}

      {#if a.kind.kind === "blur"}
        {@const k = a.kind}
        <PanelSection title="Blur">
          <SliderControl
            label="Strength"
            value={k.strength * 100}
            min={0}
            max={100}
            step={1}
            unit="%"
            description="How much the underlying pixels are softened. Applied at export."
            formatValue={(v) => `${v.toFixed(0)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "blur") return;
              updateSelected({ kind: { ...a.kind, strength: v / 100 } });
            }}
          />
          <SliderControl
            label="Corner radius"
            value={(k.radius ?? 0) * 200}
            min={0}
            max={100}
            step={1}
            unit="%"
            formatValue={(v) => `${v.toFixed(0)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "blur") return;
              updateSelected({ kind: { ...a.kind, radius: v / 200 } });
            }}
          />
          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Style</span>
            <Segmented
              size="xs"
              fill={false}
              aria-label="Blur style"
              value={k.variant}
              options={[
                { value: "glass", label: "Glass" },
                { value: "white", label: "White" },
                { value: "black", label: "Black" },
                { value: "color", label: "Color" },
              ]}
              onValueChange={(v) => {
                if (a.kind.kind !== "blur") return;
                store.pushUndoState();
                updateSelected({
                  kind: {
                    ...a.kind,
                    variant: v as "glass" | "white" | "black" | "color",
                  },
                });
              }}
            />
          </div>
          {#if k.variant === "color"}
            <ColorField
              label="Tint"
              value={k.tintColor}
              swatches={STROKE_SWATCHES}
              {recents}
              oncommit={(c: string) => {
                if (a.kind.kind !== "blur") return;
                store.pushUndoState();
                updateSelected({ kind: { ...a.kind, tintColor: c } });
                rememberColor(c);
              }}
            />
          {/if}
        </PanelSection>
      {/if}

      {#if a.kind.kind === "image"}
        {@const k = a.kind}
        <PanelSection title="Image">
          <div class="flex flex-col gap-2.5">
            <div class="flex items-center gap-2">
              <span
                class="min-w-0 flex-1 truncate text-[11px] text-muted-foreground"
                title={k.path}
              >
                {imageFileName(k.path)}
              </span>
              <Button size="xs" variant="outline" onclick={replaceImage}>Replace</Button>
            </div>
            <SliderControl
              label="Corner radius"
              value={(k.radius ?? 0) * 200}
              min={0}
              max={100}
              step={1}
              unit="%"
              formatValue={(v) => `${v.toFixed(0)}%`}
              onstart={() => store.pushUndoState()}
              onchange={(v) => {
                if (a.kind.kind !== "image") return;
                updateSelected({ kind: { ...a.kind, radius: v / 200 } });
              }}
            />
          </div>
        </PanelSection>
      {/if}

      <AnnotationAppearance {store} annotation={a} />

      {#if a.kind.kind === "rect"}
        {@const k = a.kind}
        <PanelSection title="Shape">
          <SliderControl
            label="Corner radius"
            value={(k.radius ?? 0) * 200}
            min={0}
            max={100}
            step={1}
            unit="%"
            formatValue={(v) => `${v.toFixed(0)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "rect") return;
              updateSelected({ kind: { ...a.kind, radius: v / 200 } });
            }}
          />
        </PanelSection>
      {/if}

      {#if a.kind.kind === "arrow"}
        {@const k = a.kind}
        <PanelSection title="Arrowhead">
          <SliderControl
            label="Head size"
            value={k.headSize * 100}
            min={5}
            max={40}
            step={1}
            unit="%"
            description="Length of the arrowhead as a percentage of the line."
            formatValue={(v) => `${v.toFixed(0)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "arrow") return;
              updateSelected({ kind: { ...a.kind, headSize: v / 100 } });
            }}
          />
        </PanelSection>
      {/if}

      <PanelSection title="Timing" collapsible defaultOpen>
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
          max={Math.max(regionMaxRamp(a), 0.01)}
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
          max={Math.max(regionMaxRamp(a), 0.01)}
          step={0.01}
          unit="s"
          formatValue={(v) => `${v.toFixed(2)}s`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => updateSelected({ rampOut: v })}
        />
      </PanelSection>

      <PanelSection title="Fade curves" collapsible defaultOpen={false}>
        {#snippet action()}
          <Button variant="ghost" size="xs" onclick={resetCurves}>Reset</Button>
        {/snippet}
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] font-medium text-muted-foreground">
              Editing the fade-{customCurve} curve
            </span>
            <SegmentedToggle
              checked={customCurve === "out"}
              offLabel="In"
              onLabel="Out"
              size="xs"
              aria-label="Edit fade-in or fade-out curve"
              onCheckedChange={(next) => (customCurve = next ? "out" : "in")}
            />
          </div>
          <BezierEditor
            value={customCurve === "in" ? a.easeIn : a.easeOut}
            onchange={(v) => {
              // BezierEditor streams onchange per pointermove; coalesce so a
              // whole curve drag is one undo entry, not one per frame.
              store.pushUndoStateCoalesced(`anno-curve-${a.id}-${customCurve}`, 500);
              updateSelected(customCurve === "in" ? { easeIn: v } : { easeOut: v });
            }}
            showPresets={false}
            size={220}
          />
        </div>
      </PanelSection>

      <AnnotationGeometry {store} annotation={a} />
    </div>
  {:else if store.annotations.length > 0}
    <p
      class="rounded-xl border border-dashed border-border/70 bg-card/40 px-3 py-3 text-center text-[10px] text-muted-foreground"
    >
      Select a layer to edit its appearance, timing, and geometry.
    </p>
  {/if}
</div>
