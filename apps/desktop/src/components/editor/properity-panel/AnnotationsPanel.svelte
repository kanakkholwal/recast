<script lang="ts">
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
    ImageIcon,
    Lock,
    MousePointer2,
    Pencil,
    Square,
    SquareDashedMousePointer,
    Trash2,
    Type as TypeIcon,
  } from "@lucide/svelte";
  import {
    getRecentColors,
    pushRecentColor,
  } from "$lib/annotations/recent-colors";
  import { Button } from "@recast/ui/button";
  import { ColorPicker } from "@recast/ui/color-picker";
  import { Kbd } from "@recast/ui/kbd";
  import * as Popover from "@recast/ui/popover";
  import { cn } from "@recast/ui/utils";
  import { onDestroy, onMount } from "svelte";
  import BezierEditor from "../_components/BezierEditor.svelte";
  import SliderControl from "../_components/SliderControl.svelte";
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

  type ToolDef = {
    id: AnnotationKindName | "select";
    label: string;
    icon: typeof Square;
    hotkey: string;
    /** Disabled tools are shown for discoverability with a "coming soon" tip. */
    disabled?: boolean;
  };

  // Tool palette. Disabled entries (image, polygon, blur) appear so users see
  // the roadmap; toggling them is a no-op.
  const tools: ToolDef[] = [
    { id: "select", label: "Select", icon: MousePointer2, hotkey: "V" },
    { id: "rect", label: "Rectangle", icon: Square, hotkey: "R" },
    { id: "ellipse", label: "Ellipse", icon: Circle, hotkey: "O" },
    { id: "arrow", label: "Arrow", icon: ArrowUpRight, hotkey: "A" },
    { id: "text", label: "Text", icon: TypeIcon, hotkey: "T" },
    { id: "blur", label: "Blur", icon: Droplets, hotkey: "B" },
    { id: "image", label: "Image", icon: ImageIcon, hotkey: "I", disabled: true },
  ];

  function setTool(id: ToolDef["id"], disabled?: boolean) {
    if (disabled) return;
    if (id === "select") {
      store.annotationTool = null;
      return;
    }
    store.annotationTool = store.annotationTool === id ? null : id;
  }

  // Tool hotkeys. Suppressed when focus is in an editable element so typing
  // in a text annotation or any input doesn't switch tools.
  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    if (target.isContentEditable) return true;
    const tag = target.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }

  function handleHotkey(event: KeyboardEvent) {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (isEditableTarget(event.target)) return;
    const key = event.key.toLowerCase();
    const tool = tools.find((t) => t.hotkey.toLowerCase() === key);
    if (!tool) return;
    if (tool.disabled) return;
    event.preventDefault();
    setTool(tool.id);
  }

  onMount(() => {
    window.addEventListener("keydown", handleHotkey);
  });
  onDestroy(() => {
    window.removeEventListener("keydown", handleHotkey);
  });

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

  function kindLabel(a: Annotation): string {
    switch (a.kind.kind) {
      case "rect":
        return "Rectangle";
      case "ellipse":
        return "Ellipse";
      case "arrow":
        return "Arrow";
      case "text":
        return a.kind.content.trim().slice(0, 32) || "Text";
      case "image":
        return "Image";
      case "blur":
        return "Blur";
    }
  }
  function kindIcon(a: Annotation): typeof Square {
    switch (a.kind.kind) {
      case "rect":
        return Square;
      case "ellipse":
        return Circle;
      case "arrow":
        return ArrowUpRight;
      case "text":
        return TypeIcon;
      case "image":
        return ImageIcon;
      case "blur":
        return Droplets;
    }
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

  // Curated text-overlay font whitelist. All variable fonts already loaded
  // via @fontsource-variable/* in app.css, plus generic system fallbacks.
  const FONT_FAMILIES = [
    { value: "'Geist Variable', system-ui, sans-serif", label: "Geist" },
    {
      value: "'Geist Mono Variable', ui-monospace, monospace",
      label: "Geist Mono",
    },
    {
      value: "'Google Sans Variable', system-ui, sans-serif",
      label: "Google Sans",
    },
    { value: "system-ui, sans-serif", label: "System" },
    { value: "ui-serif, Georgia, serif", label: "Serif" },
    { value: "ui-monospace, monospace", label: "Monospace" },
  ];

  const FONT_WEIGHTS: { value: 400 | 500 | 600 | 700; label: string }[] = [
    { value: 400, label: "R" },
    { value: 500, label: "M" },
    { value: 600, label: "SB" },
    { value: 700, label: "B" },
  ];

  function maxRamp(a: Annotation): number {
    return Math.max(0, (a.end - a.start) * 0.5);
  }

  // Tool status hint shown beneath the palette while a tool is active.
  const toolHint = $derived.by(() => {
    switch (store.annotationTool) {
      case "rect":
      case "ellipse":
        return "Drag on the preview to draw a shape.";
      case "arrow":
        return "Drag from start to end on the preview.";
      case "text":
        return "Drag a box on the preview, then type.";
      case "blur":
        return "Drag a region to obscure — applied at export.";
      default:
        return "";
    }
  });
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- Tool palette -->
  <PanelSection
    title="Tools"
    hint="Pick a tool, then drag on the preview. Annotations are anchored in video-space so they follow zoom and crop. Press Esc to cancel placement. Hold Alt while dragging to bypass snap."
    flush
  >
    {#snippet action()}
      <button
        type="button"
        aria-pressed={store.annotationSnapEnabled}
        onclick={() => (store.annotationSnapEnabled = !store.annotationSnapEnabled)}
        title="Toggle snap (Alt while dragging bypasses)"
        class={cn(
          "rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider transition-colors",
          store.annotationSnapEnabled
            ? "bg-primary/10 text-primary"
            : "bg-muted/40 text-muted-foreground hover:text-foreground",
        )}
      >
        Snap
      </button>
    {/snippet}
    <div class="grid grid-cols-6 gap-1">
      {#each tools as tool (tool.id)}
        {@const Icon = tool.icon}
        {@const isActive =
          tool.id === "select"
            ? store.annotationTool === null
            : store.annotationTool === tool.id}
        <button
          type="button"
          aria-pressed={isActive}
          aria-disabled={tool.disabled}
          disabled={tool.disabled}
          onclick={() => setTool(tool.id, tool.disabled)}
          title={tool.disabled
            ? `${tool.label} — coming soon`
            : `${tool.label} (${tool.hotkey})`}
          class={cn(
            "group relative flex h-10 flex-col items-center justify-center gap-0.5 rounded-md border text-[9px] font-medium transition-colors",
            "focus:outline-none focus:ring-1 focus:ring-ring",
            tool.disabled
              ? "border-dashed border-border text-muted-foreground/40 cursor-not-allowed"
              : isActive
                ? "border-primary bg-primary/10 text-primary"
                : "border-border bg-background text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon size={14} />
          <span class="leading-none">{tool.label}</span>
          {#if tool.disabled}
            <Lock
              size={8}
              class="absolute right-0.5 top-0.5 text-muted-foreground/50"
            />
          {/if}
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

  <!-- Annotation list -->
  {#if store.annotations.length === 0}
    <div
      class="flex flex-col items-center gap-2 rounded-md border border-dashed border-border bg-card/40 px-3 py-6 text-center"
    >
      <SquareDashedMousePointer size={18} class="text-muted-foreground" />
      <p class="text-[11px] font-medium text-foreground">No annotations yet</p>
      <p class="text-[10px] text-muted-foreground">
        Pick a tool above, then drag on the preview.
      </p>
    </div>
  {:else}
    <AnnotationLayerPanel {store} />
  {/if}

  <!-- Selected annotation editor -->
  {#if selected}
    {@const a = selected}
    <div class="flex flex-col gap-3 border-t border-border pt-3">
      <PanelSection title={kindLabel(a)}>
        {#snippet action()}
          <Button
            variant="destructive_soft"
            size="xs"
            class="gap-1.5"
            onclick={() => store.removeAnnotation(a.id)}
          >
            <Trash2 size={11} />
            Delete
          </Button>
        {/snippet}
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
      </PanelSection>

      <!-- Fade curves -->
      <PanelSection title="Fade curves" flush>
        {#snippet action()}
          <Button variant="ghost" size="xs" onclick={resetCurves}>Reset</Button>
        {/snippet}
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
      </PanelSection>

      <!-- Appearance: stroke (with style + custom picker), fill, glow, opacity -->
      <AnnotationAppearance {store} annotation={a} />

      <!-- Geometry: numeric inputs + frame-relative alignment -->
      <AnnotationGeometry {store} annotation={a} />

      <!-- Per-kind specific properties -->
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

      {#if a.kind.kind === "blur"}
        <PanelSection title="Blur">
          <SliderControl
            label="Strength"
            value={a.kind.strength * 100}
            min={0}
            max={100}
            step={1}
            unit="%"
            description="Controls how much the underlying pixels are softened. Applied at export."
            formatValue={(v) => `${v.toFixed(0)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "blur") return;
              updateSelected({ kind: { ...a.kind, strength: v / 100 } });
            }}
          />
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
              if (a.kind.kind !== "blur") return;
              updateSelected({ kind: { ...a.kind, radius: v / 1000 } });
            }}
          />
          <div class="space-y-1">
            <span
              class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
            >
              Style
            </span>
            <div class="grid grid-cols-4 gap-1">
              {#each [
                { id: "glass", label: "Glass", swatch: "bg-gradient-to-br from-white/40 to-blue-200/30" },
                { id: "white", label: "White", swatch: "bg-white" },
                { id: "black", label: "Black", swatch: "bg-black" },
                { id: "color", label: "Color", swatch: "bg-gradient-to-br from-rose-400 via-amber-300 to-emerald-400" },
              ] as opt (opt.id)}
                {@const sel = a.kind.kind === "blur" && a.kind.variant === opt.id}
                <button
                  type="button"
                  aria-pressed={sel}
                  onclick={() => {
                    if (a.kind.kind !== "blur") return;
                    store.pushUndoState();
                    updateSelected({
                      kind: {
                        ...a.kind,
                        variant: opt.id as "glass" | "white" | "black" | "color",
                      },
                    });
                  }}
                  class={cn(
                    "group flex flex-col items-center gap-1 rounded-md border px-1 py-1.5 transition-all duration-150",
                    sel
                      ? "border-primary/50 bg-primary/8 ring-1 ring-primary/30"
                      : "border-border/40 bg-background/40 hover:border-border",
                  )}
                >
                  <span
                    class={cn(
                      "h-4 w-full rounded border border-border/50",
                      opt.swatch,
                    )}
                  ></span>
                  <span
                    class={cn(
                      "text-[10px] font-semibold",
                      sel ? "text-primary" : "text-foreground",
                    )}
                  >
                    {opt.label}
                  </span>
                </button>
              {/each}
            </div>
          </div>
          {#if a.kind.variant === "color"}
            <Popover.Root>
              <Popover.Trigger>
                {#snippet child({ props })}
                  <button
                    type="button"
                    {...props}
                    aria-label="Choose blur tint color"
                    class="flex w-full items-center gap-2 rounded-md border border-border bg-background px-2 py-1.5 text-left transition-colors hover:border-ring"
                  >
                    <span
                      class="size-5 shrink-0 rounded border border-input"
                      style:background={a.kind.tintColor}
                    ></span>
                    <span class="min-w-0 flex-1">
                      <span class="block text-[11px] font-medium text-foreground"
                        >Tint</span
                      >
                      <span
                        class="block truncate font-mono text-[10px] text-muted-foreground"
                      >
                        {a.kind.tintColor.toUpperCase()}
                      </span>
                    </span>
                  </button>
                {/snippet}
              </Popover.Trigger>
              <Popover.Content align="start" class="w-auto p-0">
                <ColorPicker
                  value={a.kind.tintColor}
                  recents={recents}
                  oncommit={(c: string) => {
                    if (a.kind.kind !== "blur") return;
                    store.pushUndoState();
                    updateSelected({ kind: { ...a.kind, tintColor: c } });
                    rememberColor(c);
                  }}
                />
              </Popover.Content>
            </Popover.Root>
          {/if}
        </PanelSection>
      {/if}

      {#if a.kind.kind === "arrow"}
        <SliderControl
          label="Head size"
          value={a.kind.headSize * 100}
          min={5}
          max={40}
          step={1}
          unit="%"
          description="Length of the arrowhead as a percentage of the line."
          formatValue={(v) => `${v.toFixed(0)}%`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => {
            if (a.kind.kind !== "arrow") return;
            updateSelected({
              kind: { ...a.kind, headSize: v / 100 },
            });
          }}
        />
      {/if}

      {#if a.kind.kind === "text"}
        <PanelSection title="Text">
          <label
            class="flex flex-col gap-1 text-[10px] text-muted-foreground"
          >
            <span>Content</span>
            <textarea
              class="rounded-md border border-border bg-background px-2 py-1.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              rows={2}
              value={a.kind.content}
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
            ></textarea>
          </label>

          <label
            class="flex flex-col gap-1 text-[10px] text-muted-foreground"
          >
            <span>Font</span>
            <select
              class="rounded-md border border-border bg-background px-2 py-1 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              value={a.kind.fontFamily}
              onchange={(e) => {
                if (a.kind.kind !== "text") return;
                store.pushUndoState();
                updateSelected({
                  kind: {
                    ...a.kind,
                    fontFamily: (e.currentTarget as HTMLSelectElement).value,
                  },
                });
              }}
            >
              {#each FONT_FAMILIES as font (font.value)}
                <option value={font.value}>{font.label}</option>
              {/each}
            </select>
          </label>

          <SliderControl
            label="Size"
            value={a.kind.fontSize * 100}
            min={2}
            max={20}
            step={0.5}
            unit="%"
            description="Percentage of canvas height."
            formatValue={(v) => `${v.toFixed(1)}%`}
            onstart={() => store.pushUndoState()}
            onchange={(v) => {
              if (a.kind.kind !== "text") return;
              updateSelected({
                kind: { ...a.kind, fontSize: v / 100 },
              });
            }}
          />

          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Weight</span>
            <div
              class="flex items-center gap-0.5 rounded-md border border-border bg-muted/30 p-0.5"
            >
              {#each FONT_WEIGHTS as weight (weight.value)}
                {@const isActive = a.kind.kind === "text" &&
                  a.kind.fontWeight === weight.value}
                <button
                  type="button"
                  aria-pressed={isActive}
                  onclick={() => {
                    if (a.kind.kind !== "text") return;
                    store.pushUndoState();
                    updateSelected({
                      kind: { ...a.kind, fontWeight: weight.value },
                    });
                  }}
                  class={cn(
                    "h-6 min-w-6 rounded px-1.5 text-[10px] font-mono",
                    isActive
                      ? "bg-card text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  style="font-weight: {weight.value}"
                >
                  {weight.label}
                </button>
              {/each}
            </div>
          </div>

          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">Align</span>
            <div
              class="flex items-center gap-0.5 rounded-md border border-border bg-muted/30 p-0.5"
            >
              {#each [{ id: "left", icon: AlignLeft }, { id: "center", icon: AlignCenter }, { id: "right", icon: AlignRight }] as opt (opt.id)}
                {@const Icon = opt.icon}
                {@const isActive = a.kind.kind === "text" &&
                  a.kind.align === opt.id}
                <button
                  type="button"
                  aria-pressed={isActive}
                  aria-label={opt.id}
                  onclick={() => {
                    if (a.kind.kind !== "text") return;
                    store.pushUndoState();
                    updateSelected({
                      kind: {
                        ...a.kind,
                        align: opt.id as "left" | "center" | "right",
                      },
                    });
                  }}
                  class={cn(
                    "flex h-6 w-6 items-center justify-center rounded",
                    isActive
                      ? "bg-card text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <Icon size={12} />
                </button>
              {/each}
            </div>
          </div>

          <div>
            <p class="text-[10px] text-muted-foreground mb-1">Color</p>
            <div class="flex flex-wrap gap-1">
              {#each STROKE_SWATCHES as swatch (swatch)}
                {@const isActive = a.kind.kind === "text" &&
                  a.kind.color === swatch}
                <button
                  type="button"
                  aria-label="Color {swatch}"
                  aria-pressed={isActive}
                  onclick={() => {
                    if (a.kind.kind !== "text") return;
                    store.pushUndoState();
                    updateSelected({
                      kind: { ...a.kind, color: swatch },
                    });
                  }}
                  class={cn(
                    "size-5 rounded-full border-2 transition",
                    isActive
                      ? "border-ring ring-1 ring-ring"
                      : "border-border",
                  )}
                  style:background={swatch}
                ></button>
              {/each}
            </div>
          </div>
        </PanelSection>
      {/if}
    </div>
  {:else if store.annotations.length > 0}
    <p class="rounded-md border border-dashed border-border bg-card/40 px-3 py-3 text-center text-[10px] text-muted-foreground">
      Select an annotation to edit its timing, curves, and appearance.
    </p>
  {/if}
</div>
