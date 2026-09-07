<script lang="ts">
import { AiAtom } from "@recast/icons";
import { ColorField } from "@recast/ui/color-field";
import { ColorPicker } from "@recast/ui/color-picker";
import * as Popover from "@recast/ui/popover";
import { SegmentedToggle } from "@recast/ui/segmented";
import { cn } from "@recast/ui/utils";
import {
	hasFill as kindHasFill,
	hasStroke as kindHasStroke,
} from "../../../lib/annotations/kind-groups";
import { FILL_SWATCHES, STROKE_SWATCHES } from "../../../lib/annotations/palette";
import { getRecentColors, pushRecentColor } from "../../../lib/annotations/recent-colors";
import type {
	Annotation,
	AnnotationGlow,
	AnnotationStrokeStyle,
	EditorStore,
} from "../../../stores/editor-store.svelte";
import PanelSection from "../PanelSection.svelte";
import PropRow from "../PropRow.svelte";
import PropSelect from "../PropSelect.svelte";
import SliderRow from "../SliderRow.svelte";
import { defaultGlow } from "./annotation-appearance.logic";

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
	const base = defaultGlow(annotation.glow, annotation.stroke.color);
	store.updateAnnotation(annotation.id, {
		glow: { ...base, ...update },
	});
}

const hasStroke = $derived(kindHasStroke(annotation.kind.kind));
const hasFill = $derived(kindHasFill(annotation.kind.kind));
</script>

<PanelSection
  title="Appearance"
  hint="Stroke styles show in preview. Glow previews only; exports use a solid stroke for now."
  flush
>
  <div class="flex flex-col gap-3">
    {#if hasStroke}
      <div class="space-y-2">
        <SliderRow
          label="Width"
          value={annotation.stroke.width * 1000}
          min={0}
          max={20}
          step={1}
          unit="‰"
          formatValue={(v) => `${v.toFixed(0)}‰`}
          onstart={() => store.pushUndoState()}
          onchange={(v) => setStroke({ width: v / 1000 })}
        />

        <PropRow label="Style">
          <PropSelect
            class="flex-1"
            label="Stroke style"
            value={annotation.stroke.style ?? "solid"}
            options={STROKE_STYLES}
            onChange={(v) => {
              store.pushUndoState();
              setStroke({ style: v as AnnotationStrokeStyle });
            }}
          />
        </PropRow>

        <PropRow label="Color">
          <ColorField
            dense
            hideLabel
            class="flex-1"
            label="Stroke color"
            value={annotation.stroke.color}
            swatches={STROKE_SWATCHES}
            {recents}
            oncommit={(c: string) => {
              store.pushUndoState();
              setStrokeColor(c);
            }}
          />
        </PropRow>
      </div>
    {/if}

    {#if hasFill}
      <div class="space-y-1.5">
        <span class="text-[11px] text-muted-foreground">Fill</span>
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

    <SliderRow
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

    <div class="space-y-2">
      <div class="flex items-center justify-between gap-2">
        <span class="inline-flex items-center gap-1.5 text-[11px] font-medium text-foreground">
          <AiAtom size={11} class="text-muted-foreground" />
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
        <PropRow label="Color">
          <ColorField
            dense
            hideLabel
            class="flex-1"
            label="Glow color"
            value={g.color}
            {recents}
            oncommit={(c: string) => {
              store.pushUndoState();
              setGlow({ color: c });
              rememberColor(c);
            }}
          />
        </PropRow>
        <SliderRow
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
        <SliderRow
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
