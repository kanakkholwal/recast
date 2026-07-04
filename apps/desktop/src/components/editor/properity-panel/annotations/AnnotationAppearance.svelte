<script lang="ts">
  import { FILL_SWATCHES, STROKE_SWATCHES } from "$lib/annotations/palette";
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
  import { ColorPicker } from "@recast/ui/color-picker";
  import * as Popover from "@recast/ui/popover";
  import { Segmented } from "@recast/ui/segmented";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";
  import PanelSection from "../PanelSection.svelte";

  interface Props {
    store: EditorStore;
    annotation: Annotation;
  }

  let { store, annotation }: Props = $props();

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

  // Images get a border (stroke) too — width/style/colour frame the image.
  const hasStroke = $derived(isShape || annotation.kind.kind === "image");

  const hasFill = $derived(
    annotation.kind.kind === "rect" || annotation.kind.kind === "ellipse",
  );
</script>

<PanelSection
  title="Appearance"
  hint="Stroke styles show in preview. Glow previews only; exports use a solid stroke for now."
  flush
>
  <div class="flex flex-col gap-3">
    {#if hasStroke}
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

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Style</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Stroke style"
            value={annotation.stroke.style ?? "solid"}
            options={STROKE_STYLES}
            onValueChange={(v) => {
              store.pushUndoState();
              setStroke({ style: v as AnnotationStrokeStyle });
            }}
          />
        </div>

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
                isActive ? "border-foreground shadow-sm" : "border-border/40 hover:border-border",
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
                  class="grid size-5 place-items-center rounded-full border-2 border-dashed border-border/60 text-[11px] leading-none text-muted-foreground transition hover:border-border hover:text-foreground"
                >
                  +
                </button>
              {/snippet}
            </Popover.Trigger>
            <Popover.Content align="start" class="w-auto p-0">
              <ColorPicker
                value={annotation.stroke.color}
                {recents}
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
      <div class="space-y-1.5">
        <span class="text-[10px] text-muted-foreground">Fill</span>
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
                "size-5 overflow-hidden rounded-md border-2 transition",
                isActive ? "border-foreground shadow-sm" : "border-border/40 hover:border-border",
                swatch === "transparent" && "bg-background",
              )}
              style:background={swatch === "transparent" ? undefined : swatch}
            >
              {#if swatch === "transparent"}
                <span
                  class="block h-full w-full"
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
                  class="grid size-5 place-items-center rounded-md border-2 border-dashed border-border/60 text-[11px] leading-none text-muted-foreground transition hover:border-border hover:text-foreground"
                >
                  +
                </button>
              {/snippet}
            </Popover.Trigger>
            <Popover.Content align="start" class="w-auto p-0">
              <ColorPicker
                value={annotation.fill && annotation.fill !== "transparent" ? annotation.fill : "#3b82f633"}
                {recents}
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

    <div
      class="space-y-2 rounded-xl border border-border/60 bg-card/40 p-2 shadow-(--shadow-craft-inset)"
    >
      <div class="flex items-center justify-between gap-2">
        <span class="inline-flex items-center gap-1.5 text-[11px] font-medium text-foreground">
          <Sparkles size={11} class="text-primary" />
          Glow
        </span>
        <SegmentedToggle
          checked={!!annotation.glow}
          size="xs"
          aria-label="Glow"
          onCheckedChange={(next) => {
            store.pushUndoState();
            setGlow(next ? {} : null);
          }}
        />
      </div>
      {#if annotation.glow}
        {@const g = annotation.glow}
        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Color</span>
          <Popover.Root>
            <Popover.Trigger>
              {#snippet child({ props })}
                <button
                  type="button"
                  {...props}
                  aria-label="Glow color"
                  class="size-5 rounded-full border-2 border-border/40 shadow-sm transition hover:border-border"
                  style:background={g.color}
                ></button>
              {/snippet}
            </Popover.Trigger>
            <Popover.Content align="start" class="w-auto p-0">
              <ColorPicker
                value={g.color}
                {recents}
                oncommit={(c: string) => {
                  store.pushUndoState();
                  setGlow({ color: c });
                  rememberColor(c);
                }}
              />
            </Popover.Content>
          </Popover.Root>
        </div>
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
          {annotation.kind.kind === "arrow"
            ? "Preview only for arrows; the export drops the glow."
            : "Renders in the exported video."}
        </p>
      {/if}
    </div>
  </div>
</PanelSection>
