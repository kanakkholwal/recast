<script lang="ts">
  import {
    getRecentColors,
    pushRecentColor,
  } from "$lib/annotations/recent-colors";
  import type {
    Annotation,
    AnnotationGlow,
    AnnotationStrokeStyle,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { Sparkles } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { ColorPicker } from "@recast/ui/color-picker";
  import * as Popover from "@recast/ui/popover";
  import { cn } from "@recast/ui/utils";
  import InspectorHint from "../../InspectorHint.svelte";
  import SliderControl from "../../_components/SliderControl.svelte";

  interface Props {
    store: EditorStore;
    annotation: Annotation;
  }

  let { store, annotation }: Props = $props();

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

  const STROKE_STYLES: { value: AnnotationStrokeStyle; label: string }[] = [
    { value: "solid", label: "Solid" },
    { value: "dashed", label: "Dashed" },
    { value: "dotted", label: "Dotted" },
  ];

  let recents = $state<string[]>(getRecentColors());

  function rememberColor(color: string) {
    recents = pushRecentColor(color);
  }

  function setStroke(update: Partial<Annotation["stroke"]>) {
    store.updateAnnotation(annotation.id, {
      stroke: { ...annotation.stroke, ...update },
    });
  }

  function setStrokeColor(color: string) {
    setStroke({ color });
    rememberColor(color);
  }

  function setFill(color: string) {
    store.updateAnnotation(annotation.id, { fill: color });
    if (color !== "transparent") rememberColor(color);
  }

  function setOpacity(value01: number) {
    store.updateAnnotation(annotation.id, {
      opacity: Math.max(0, Math.min(1, value01)),
    });
  }

  function setGlow(update: Partial<AnnotationGlow> | null) {
    if (update === null) {
      store.updateAnnotation(annotation.id, { glow: undefined });
      return;
    }
    const base: AnnotationGlow = annotation.glow ?? {
      color: annotation.stroke.color || "#3b82f6",
      blur: 0.012,
      opacity: 0.7,
    };
    store.updateAnnotation(annotation.id, {
      glow: { ...base, ...update },
    });
  }

  const isShape = $derived(
    annotation.kind.kind === "rect" ||
      annotation.kind.kind === "ellipse" ||
      annotation.kind.kind === "arrow",
  );

  const hasFill = $derived(
    annotation.kind.kind === "rect" || annotation.kind.kind === "ellipse",
  );
</script>

<section class="flex flex-col gap-3 border-t border-border pt-3">
  <header class="flex items-center justify-between gap-2">
    <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
      Appearance
    </h3>
    <InspectorHint
      content="Stroke styles render in the preview. Glow is preview-only in v2 — exports fall back to a solid stroke."
    />
  </header>

  {#if isShape}
    <!-- Stroke -->
    <div class="space-y-2">
      <SliderControl
        label="Stroke width"
        value={annotation.stroke.width * 1000}
        min={0}
        max={20}
        step={1}
        unit="‰"
        formatValue={(v) => `${v.toFixed(0)}‰`}
        onstart={() => store.pushUndoState()}
        onchange={(v) => setStroke({ width: v / 1000 })}
      />

      <!-- Stroke style -->
      <div class="flex items-center justify-between gap-2">
        <span class="text-[10px] text-muted-foreground">Style</span>
        <div class="flex items-center gap-0.5 rounded-md border border-border bg-muted/30 p-0.5">
          {#each STROKE_STYLES as opt (opt.value)}
            {@const isActive = (annotation.stroke.style ?? "solid") === opt.value}
            <button
              type="button"
              aria-pressed={isActive}
              onclick={() => {
                store.pushUndoState();
                setStroke({ style: opt.value });
              }}
              class={cn(
                "h-6 px-2 rounded text-[10px] font-medium transition-colors",
                isActive
                  ? "bg-card text-foreground shadow-craft-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {opt.label}
            </button>
          {/each}
        </div>
      </div>

      <!-- Stroke color: swatches + custom picker -->
      <div class="flex flex-wrap items-center gap-1">
        {#each STROKE_SWATCHES as swatch (swatch)}
          {@const isActive = annotation.stroke.color === swatch}
          <button
            type="button"
            aria-label={`Stroke ${swatch}`}
            aria-pressed={isActive}
            onclick={() => {
              store.pushUndoState();
              setStrokeColor(swatch);
            }}
            class={cn(
              "size-5 rounded-full border-2 transition",
              isActive ? "border-ring ring-1 ring-ring" : "border-border",
            )}
            style:background={swatch}
          ></button>
        {/each}
        <Popover.Root>
          <Popover.Trigger>
            {#snippet child({ props })}
              <button
                type="button"
                {...props}
                aria-label="Custom stroke color"
                class="size-5 rounded-full border-2 border-dashed border-border text-[9px] text-muted-foreground transition hover:border-ring"
              >
                +
              </button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content align="start" class="w-auto p-0">
            <ColorPicker
              value={annotation.stroke.color}
              recents={recents}
              oncommit={(c: string) => {
                store.pushUndoState();
                setStrokeColor(c);
              }}
            />
          </Popover.Content>
        </Popover.Root>
      </div>
    </div>
  {/if}

  {#if hasFill}
    <!-- Fill -->
    <div class="space-y-2">
      <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Fill
      </p>
      <div class="flex flex-wrap items-center gap-1">
        {#each FILL_SWATCHES as swatch (swatch)}
          {@const isActive = annotation.fill === swatch}
          <button
            type="button"
            aria-label={swatch === "transparent" ? "No fill" : `Fill ${swatch}`}
            aria-pressed={isActive}
            onclick={() => {
              store.pushUndoState();
              setFill(swatch);
            }}
            class={cn(
              "size-5 rounded-md border-2 transition",
              isActive ? "border-ring ring-1 ring-ring" : "border-border",
              swatch === "transparent" && "bg-background",
            )}
            style:background={swatch === "transparent" ? undefined : swatch}
          >
            {#if swatch === "transparent"}
              <span
                class="block h-full w-full rounded-sm"
                style="background: repeating-linear-gradient(45deg, var(--color-muted) 0 3px, transparent 3px 6px);"
              ></span>
            {/if}
          </button>
        {/each}
        <Popover.Root>
          <Popover.Trigger>
            {#snippet child({ props })}
              <button
                type="button"
                {...props}
                aria-label="Custom fill color"
                class="size-5 rounded-md border-2 border-dashed border-border text-[9px] text-muted-foreground transition hover:border-ring"
              >
                +
              </button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content align="start" class="w-auto p-0">
            <ColorPicker
              value={annotation.fill && annotation.fill !== "transparent" ? annotation.fill : "#3b82f633"}
              recents={recents}
              oncommit={(c: string) => {
                store.pushUndoState();
                setFill(c);
              }}
            />
          </Popover.Content>
        </Popover.Root>
      </div>
    </div>
  {/if}

  <!-- Master opacity -->
  <SliderControl
    label="Opacity"
    value={(annotation.opacity ?? 1) * 100}
    min={0}
    max={100}
    step={1}
    unit="%"
    formatValue={(v) => `${v.toFixed(0)}%`}
    onstart={() => store.pushUndoState()}
    onchange={(v) => setOpacity(v / 100)}
  />

  <!-- Glow disclosure -->
  <div class="space-y-2 rounded-md border border-border bg-card/30 p-2">
    <div class="flex items-center justify-between gap-2">
      <span class="inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        <Sparkles size={10} />
        Glow
      </span>
      <Button
        variant={annotation.glow ? "default_soft" : "ghost"}
        size="xs"
        onclick={() => {
          store.pushUndoState();
          if (annotation.glow) setGlow(null);
          else setGlow({});
        }}
      >
        {annotation.glow ? "On" : "Off"}
      </Button>
    </div>
    {#if annotation.glow}
      {@const g = annotation.glow}
      <SliderControl
        label="Blur"
        value={g.blur * 1000}
        min={0}
        max={50}
        step={1}
        unit="‰"
        formatValue={(v) => `${v.toFixed(0)}‰`}
        onstart={() => store.pushUndoState()}
        onchange={(v) => setGlow({ blur: v / 1000 })}
      />
      <SliderControl
        label="Intensity"
        value={g.opacity * 100}
        min={0}
        max={100}
        step={1}
        unit="%"
        formatValue={(v) => `${v.toFixed(0)}%`}
        onstart={() => store.pushUndoState()}
        onchange={(v) => setGlow({ opacity: v / 100 })}
      />
      <p class="text-[10px] leading-tight text-muted-foreground">
        Preview only — exports use a solid stroke. Tracked for v2.1.
      </p>
    {/if}
  </div>
</section>
