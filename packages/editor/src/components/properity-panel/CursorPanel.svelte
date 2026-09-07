<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { Check, EyeOff, GitGraph, Spline, Target } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { SegmentedToggle } from "@recast/ui/segmented";
import { SliderControl } from "@recast/ui/slider-control";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { Image } from "@unpic/svelte";
import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";
import { EASE } from "../../lib/easing/cubic-bezier";
import { motionDuration } from "../../lib/motion.svelte";
import { registry } from "../../lib/registry";
import type { EditorStore } from "../../stores/editor-store.svelte";
import CursorTrajectoryMap from "../_components/CursorTrajectoryMap.svelte";
import InspectorHint from "../InspectorHint.svelte";
import { isCursorAnimTouched, svgSwatchUrl } from "./cursor-panel.logic";
import EasingControl from "./EasingControl.svelte";
import PanelSection from "./PanelSection.svelte";
import PropRow from "./PropRow.svelte";
import PropSelect from "./PropSelect.svelte";

// Named so the swatch announces 'Amber', not a hex; deliberately vivid, since this ring must survive busy content.
const highlightColors: { label: string; value: string }[] = [
	{ label: "Blue", value: "#3b82f6" },
	{ label: "Red", value: "#ef4444" },
	{ label: "Green", value: "#22c55e" },
	{ label: "Amber", value: "#f59e0b" },
	{ label: "Violet", value: "#8b5cf6" },
	{ label: "Pink", value: "#ec4899" },
	{ label: "Cyan", value: "#06b6d4" },
	{ label: "White", value: "#ffffff" },
];

interface Props {
	store: EditorStore;
}

let { store }: Props = $props();
let showTrajectoryMap = $state(false);

const activeStyle = $derived(registry.get("cursor", store.cursorSettings.style));

// From the registry so installed extension packs surface alongside built-ins.
const smoothingPresets = $derived(registry.list("smoothing"));

function updateCursorSettings(updates: Partial<EditorStore["cursorSettings"]>, trackUndo = false) {
	if (trackUndo) store.pushUndoState();
	store.updateCursorSettings(updates);
}

// A select, not a chip row: one active id, "custom" once values drift off every preset.
const activeSmoothingId = $derived(
	smoothingPresets.find(
		(p) =>
			p.value.smoothing === store.cursorSettings.smoothing &&
			p.value.snapToClicks === store.cursorSettings.snapToClicks &&
			p.value.snapWindowMs === store.cursorSettings.snapWindowMs,
	)?.id ?? "custom",
);
const smoothingOptions = $derived([
	{ value: "custom", label: "Custom" },
	...smoothingPresets.map((p) => ({ value: p.id, label: p.label })),
]);
function applySmoothingPreset(id: string) {
	const preset = smoothingPresets.find((p) => p.id === id);
	if (!preset) return;
	store.pushUndoState();
	store.updateCursorSettings({
		smoothing: preset.value.smoothing,
		snapToClicks: preset.value.snapToClicks,
		snapWindowMs: preset.value.snapWindowMs,
	});
}
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- Dense inline slider row: external label column, compact field-surface track. -->
  {#snippet sliderRow(cfg: {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    unit?: string;
    description?: string;
    set: (v: number) => void;
  })}
    <PropRow label={cfg.label}>
      <SliderControl
        dense
        hideLabel
        class="flex-1"
        label={cfg.label}
        value={cfg.value}
        min={cfg.min}
        max={cfg.max}
        step={cfg.step}
        unit={cfg.unit ?? ""}
        description={cfg.description}
        onstart={() => store.pushUndoState()}
        onchange={cfg.set}
      />
    </PropRow>
  {/snippet}

  <!-- Borderless toggle row: icon + label (+ hint) left, switch right. The one
       switch shape across the panel, consistent with the slider/select rows. -->
  {#snippet switchRow(cfg: {
    icon?: IconComponent;
    label: string;
    hint?: string;
    checked: boolean;
    onchange: (next: boolean) => void;
  })}
    <div class="flex items-center justify-between gap-2">
      <span
        class="inline-flex items-center gap-1.5 text-[11px] font-medium text-foreground"
      >
        {#if cfg.icon}
          {@const Icon = cfg.icon}
          <Icon size={11} class="text-muted-foreground" />
        {/if}
        {cfg.label}
        {#if cfg.hint}
          <InspectorHint content={cfg.hint} />
        {/if}
      </span>
      <SegmentedToggle
        checked={cfg.checked}
        size="xs"
        aria-label={cfg.label}
        onCheckedChange={cfg.onchange}
      />
    </div>
  {/snippet}

  {@render switchRow({
    label: "Show cursor",
    checked: store.cursorSettings.enabled,
    onchange: (next) => updateCursorSettings({ enabled: next }, true),
  })}

  {#if store.cursorSettings.enabled}
    <PanelSection
      title="Style"
      hint="Pick a cursor style and size. Styles render in the preview and the export alike; pointer styles also swap art for press, right-click, and drag."
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
        class="grid grid-cols-4 gap-1 rounded-lg border border-border/60 bg-muted/30 p-1 shadow-(--shadow-craft-inset)"
      >
        {#each registry.list("cursor") as style (style.id)}
          {@const isActive = store.cursorSettings.style === style.id}
          <button
            type="button"
            aria-pressed={isActive}
            aria-label={`${style.label} cursor`}
            onclick={() => {
              store.pushUndoState();
              store.updateCursorSettings({ style: style.id });
            }}
            title={style.description
              ? `${style.label}: ${style.description}`
              : style.label}
            class={cn(
              "inline-flex items-center justify-center group relative aspect-square overflow-hidden rounded-md border transition-all duration-150",
              "focus:outline-none focus:ring-2 focus:ring-ring/40",
              isActive
                ? "border-foreground/40 bg-foreground/10 text-foreground"
                : "border-transparent bg-background/40 text-foreground/80 hover:border-border hover:bg-background/80 hover:text-foreground",
            )}
          >
            <Image
              src={svgSwatchUrl(style.value.svg)}
              alt={style.label}
              draggable="false"
              class="size-10 shadow-(0_0_0_1px_color-mix(in_srgb,var(--color-foreground)_85%,transparent))"
              layout="constrained"
              aria-hidden="true"
            />

            {#if isActive}
              <span
                aria-hidden="true"
                class="pointer-events-none absolute right-0.5 top-0.5 size-1.5 rounded-full bg-foreground shadow-[0_0_0_1.5px_color-mix(in_srgb,var(--color-background)_85%,transparent)]"
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

      <div class="mt-2.5">
        {@render sliderRow({
          label: "Size",
          value: store.cursorSettings.size,
          min: 1,
          max: 15,
          step: 1,
          unit: "x",
          set: (v) => store.updateCursorSettings({ size: v }),
        })}
      </div>
    </PanelSection>

    <PanelSection
      title="Motion"
      hint="Smooth the captured path, anchor clicks in place, and shape motion with an optional easing curve."
      flush
      collapsible
    >
      {#snippet action()}
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props as Record<string, unknown>}
                size="icon-xs"
                variant="raw"
                aria-label="Toggle trajectory map"
                aria-pressed={showTrajectoryMap}
                onclick={() => (showTrajectoryMap = !showTrajectoryMap)}
              >
                <GitGraph size={11} class="text-muted-foreground" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>
            {showTrajectoryMap ? "Hide" : "Show"} trajectory map
          </Tooltip.Content>
        </Tooltip.Root>
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

        <PropRow label="Preset">
          <PropSelect
            class="flex-1"
            label="Smoothing preset"
            value={activeSmoothingId}
            options={smoothingOptions}
            onChange={applySmoothingPreset}
          />
        </PropRow>

        {@render sliderRow({
          label: "Smoothing",
          value: store.cursorSettings.smoothing,
          min: 0,
          max: 100,
          step: 5,
          unit: "%",
          description:
            store.cursorSettings.smoothing === 0
              ? "Off (cursor follows the raw capture)"
              : undefined,
          set: (v) => store.updateCursorSettings({ smoothing: v }),
        })}

        {@render switchRow({
          icon: Target,
          label: "Snap to clicks",
          hint: "Around every mouse-down, pin the smoothed curve to the exact click x/y inside the snap window. Prevents smoothing from rounding the corner off a press target.",
          checked: store.cursorSettings.snapToClicks,
          onchange: (next) => updateCursorSettings({ snapToClicks: next }, true),
        })}

        {#if store.cursorSettings.snapToClicks}
          {@render sliderRow({
            label: "Snap window",
            value: store.cursorSettings.snapWindowMs,
            min: 0,
            max: 200,
            step: 10,
            unit: "ms",
            description: "Half-width of the cosine-ramped anchor around each click.",
            set: (v) => store.updateCursorSettings({ snapWindowMs: v }),
          })}
        {/if}

        <!-- Motion easing: opt-in, presets-first with a hidden custom graph -->
        <div class="space-y-2">
          {@render switchRow({
            icon: Spline,
            label: "Motion easing",
            hint: "Reshape how the cursor interpolates between captured samples. Default (off) preserves the raw trajectory; ease-out curves decelerate into rest. Preview only.",
            checked: !!store.cursorMotionEasing,
            onchange: (next) =>
              (store.cursorMotionEasing = next ? { ...EASE } : null),
          })}

          {#if store.cursorMotionEasing}
            <EasingControl
              value={store.cursorMotionEasing}
              onpick={(next) => {
                // The setter pushes its own undo entry; a second push here made one Ctrl+Z look like a no-op.
                store.cursorMotionEasing = next;
              }}
              ondrag={(next) => store.setCursorMotionEasingLive(next)}
            />

            <p class="text-[10px] leading-snug text-muted-foreground">
              Applies to preview only.
            </p>
          {/if}
        </div>
      </div>
    </PanelSection>

    <PanelSection
      title="Animation"
      hint="Bounce reacts to clicks, sway adds life at rest, and motion blur trails fast movement. These render at export only — the preview will not change as you drag."
      collapsible
    >
      {#snippet action()}
        {#if isCursorAnimTouched(store.cursorSettings)}
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
      {@render sliderRow({
        label: "Bounce",
        description: "How much the cursor squashes when you click",
        value: store.cursorSettings.clickBounce,
        min: 0,
        max: 5,
        step: 0.05,
        unit: "x",
        set: (v) => store.updateCursorSettings({ clickBounce: v }),
      })}

      {#if store.cursorSettings.clickBounce > 0}
        <span
          class="block"
          in:fly={{ y: 4, duration: motionDuration(180), easing: cubicOut }}
        >
          {@render sliderRow({
            label: "Speed",
            description: "Length of the bounce window",
            value: store.cursorSettings.bounceSpeedMs,
            min: 80,
            max: 500,
            step: 10,
            unit: " ms",
            set: (v) => store.updateCursorSettings({ bounceSpeedMs: v }),
          })}
        </span>
      {/if}

      {@render sliderRow({
        label: "Sway",
        description: "Subtle wobble during slow motion. Fades as you move faster.",
        value: store.cursorSettings.sway,
        min: 0,
        max: 1,
        step: 0.01,
        unit: "x",
        set: (v) => store.updateCursorSettings({ sway: v }),
      })}

      {@render sliderRow({
        label: "Blur",
        description: "Velocity-proportional trail behind fast cursor movement",
        value: store.cursorSettings.motionBlur,
        min: 0,
        max: 1,
        step: 0.01,
        unit: "x",
        set: (v) => store.updateCursorSettings({ motionBlur: v }),
      })}
    </PanelSection>

    <PanelSection
      title="Click highlight"
      hint="Useful for tutorials and product demos where click targets should be obvious."
      flush
      collapsible
      defaultOpen={store.cursorSettings.highlightClicks}
    >
      {#snippet action()}
        <SegmentedToggle
          checked={store.cursorSettings.highlightClicks}
          size="xs"
          aria-label="Click highlight"
          onCheckedChange={(next) =>
            updateCursorSettings({ highlightClicks: next }, true)}
        />
      {/snippet}

      {#if store.cursorSettings.highlightClicks}
        <div
          class="grid grid-cols-8 gap-1"
          in:fade={{ duration: motionDuration(140) }}
        >
          {#each highlightColors as swatch (swatch.value)}
            {@const isSelected =
              store.cursorSettings.highlightColor === swatch.value}
            <Button
              variant="raw"
              size="raw"
              onclick={() =>
                updateCursorSettings(
                  { highlightColor: swatch.value },
                  store.cursorSettings.highlightColor !== swatch.value,
                )}
              title={swatch.label}
              aria-label="Use {swatch.label} click highlight"
              aria-pressed={isSelected}
              class={cn(
                "group relative aspect-square w-full overflow-hidden rounded-md border transition-all",
                isSelected
                  ? "border-foreground/60 ring-2 ring-foreground/25"
                  : "border-border hover:border-foreground/30",
              )}
              style="background-color: {swatch.value}"
            >
              {#if isSelected}
                <!-- Carries its own contrast so the mark reads on the white swatch too. -->
                <span
                  class="absolute inset-0 grid place-items-center"
                  aria-hidden="true"
                >
                  <span
                    class="grid size-3.5 place-items-center rounded-full bg-foreground text-background shadow-sm"
                  >
                    <Check class="size-2.5" />
                  </span>
                </span>
              {/if}
            </Button>
          {/each}
        </div>

        <div class="mt-2.5">
          {@render sliderRow({
            label: "Opacity",
            value: store.cursorSettings.highlightOpacity,
            min: 10,
            max: 100,
            step: 5,
            unit: "%",
            set: (v) => store.updateCursorSettings({ highlightOpacity: v }),
          })}
        </div>
      {/if}
    </PanelSection>

    <PanelSection
      title="Idle"
      hint="Hide the cursor after inactivity for cleaner sections without interaction."
      flush
      collapsible
      defaultOpen={store.cursorSettings.hideWhenIdle}
    >
      {#snippet action()}
        <SegmentedToggle
          checked={store.cursorSettings.hideWhenIdle}
          size="xs"
          aria-label="Hide cursor when idle"
          onCheckedChange={(next) =>
            updateCursorSettings({ hideWhenIdle: next }, true)}
        />
      {/snippet}
      {#if store.cursorSettings.hideWhenIdle}
        {@render sliderRow({
          label: "Timeout",
          value: store.cursorSettings.idleTimeout,
          min: 1,
          max: 10,
          step: 1,
          unit: "s",
          set: (v) => store.updateCursorSettings({ idleTimeout: v }),
        })}
      {/if}
    </PanelSection>
  {:else}
    <div
      class="flex items-center gap-2 rounded-md border border-dashed border-border bg-muted/20 px-3 py-2.5"
    >
      <EyeOff size={13} class="shrink-0 text-muted-foreground" />
      <p class="flex-1 text-[11px] text-muted-foreground">
        Enable it to tune style, motion, and click highlights.
      </p>
    </div>
  {/if}
</div>
