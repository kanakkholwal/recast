<script lang="ts">
import { kindIcon, kindLabel } from "$lib/annotations/kind-label";
import { clockCentis as fmtTime } from "$lib/format/time";
import { imageFileName, toolHint as toolHintFor } from "./annotations-panel.logic";
import { isOutsideClip, regionMaxRamp, retimeEnd, retimeStart } from "./focus-panel.logic";
import { FONT_WEIGHTS, STROKE_SWATCHES } from "$lib/annotations/palette";
import { getRecentColors, pushRecentColor } from "$lib/annotations/recent-colors";
import { EASE } from "$lib/easing/cubic-bezier";
import {
	DEFAULT_ANNOTATION_RAMP,
	type Annotation,
	type EditorStore,
} from "$lib/stores/editor-store.svelte";
import {
	AlignCenter,
	AlignLeft,
	AlignRight,
	SquareDashedMousePointer,
	Trash2,
} from "@recast/icons";
import { toast } from "@recast/ui/sonner";
import { pickImageFile } from "$lib/annotations/image-import";
import type { TitlePreset } from "$lib/annotations/title-presets";
import { Button } from "@recast/ui/button";
import { ColorField } from "@recast/ui/color-field";
import { Kbd } from "@recast/ui/kbd";
import { Segmented } from "@recast/ui/segmented";
import { SegmentedToggle } from "@recast/ui/segmented";
import FontPicker from "./FontPicker.svelte";
import { SliderControl } from "@recast/ui/slider-control";
import { Textarea } from "@recast/ui/textarea";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { motionDuration } from "$lib/motion.svelte";
import InspectorHint from "../InspectorHint.svelte";
import EasingControl from "./EasingControl.svelte";
import AnnotationAppearance from "./annotations/AnnotationAppearance.svelte";
import AnnotationGeometry from "./annotations/AnnotationGeometry.svelte";
import AnnotationLayerPanel from "./annotations/AnnotationLayerPanel.svelte";
import PanelSection from "./PanelSection.svelte";
import TitlePresetTiles from "./TitlePresetTiles.svelte";

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

// Insert a ready-styled title/lower-third: a positioned text annotation plus a
// legibility glow. The user edits the placeholder text in place.
function insertTitle(preset: TitlePreset) {
	store.annotationTool = null;
	store.addAnnotation(preset.build(), undefined, undefined, {
		glow: { ...preset.glow },
		name: preset.label,
	});
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

function updateSelected(updates: Partial<Annotation>, trackUndo = false) {
	if (!selected) return;
	if (trackUndo) store.pushUndoState();
	store.updateAnnotation(selected.id, updates);
}

// Curves only. Resetting rampIn/rampOut from here changed the Fade in / Fade
// out sliders in the Timing section above, with no hint that it would.
function resetCurves() {
	if (!selected) return;
	store.pushUndoState();
	store.updateAnnotation(selected.id, {
		easeIn: { ...EASE },
		easeOut: { ...EASE },
	});
}

function resetFades() {
	if (!selected) return;
	store.pushUndoState();
	store.updateAnnotation(selected.id, {
		rampIn: DEFAULT_ANNOTATION_RAMP,
		rampOut: DEFAULT_ANNOTATION_RAMP,
	});
}

const toolHint = $derived(toolHintFor(store.annotationTool));

// NLE accessors, matching FocusPanel: `outPoint` resolves the legacy
// `trimEnd === 0` sentinel.
const clipIn = $derived(store.inPoint);
const clipOut = $derived(store.outPoint);

// An annotation timed outside the trim never plays, and the Rust side silently
// repairs it at export ("annotation_out_of_trim" in validate_render_state), so
// it has to be visible and fixable here instead.
const outOfClip = $derived(!!selected && isOutsideClip(selected, clipIn, clipOut));

function fitToClip() {
	if (!selected) return;
	store.pushUndoState();
	store.updateAnnotation(selected.id, {
		start: Math.max(clipIn, Math.min(selected.start, clipOut - 0.1)),
		end: Math.min(clipOut, Math.max(selected.end, clipIn + 0.1)),
	});
}

const startFromPlayhead = $derived(
	selected ? retimeStart(selected, store.currentTime, clipIn) : null,
);
const endFromPlayhead = $derived(selected ? retimeEnd(selected, store.currentTime, clipOut) : null);
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- The tools themselves live on the player bar under the preview, next to the
       picture you draw on. This panel describes the SELECTION, not the mode. -->
  <PanelSection
    title="Drawing"
    hint="Pick a tool from the bar under the preview, then click to drop one or drag to draw it. Shift keeps a shape square or an arrow at 45°; annotations follow zoom and crop."
    flush
  >
    {#snippet action()}
      <div class="flex items-center gap-1">
        <span class="text-[10px] text-muted-foreground">Snap</span>
        <InspectorHint
          content="While dragging, edges and centres pull into line with the frame and with your other annotations. Hold Alt to bypass it for one drag."
        />
        <SegmentedToggle
          checked={store.annotationSnapEnabled}
          size="xs"
          aria-label="Snap to guides"
          onCheckedChange={(next) => (store.annotationSnapEnabled = next)}
        />
      </div>
    {/snippet}

    {#if toolHint}
      <p class="text-[10px] text-muted-foreground">
        {toolHint}
        <Kbd class="ml-1">Esc</Kbd>
        to cancel.
      </p>
    {/if}
  </PanelSection>

  <PanelSection
    title="Titles"
    hint="Drop in a styled title, subtitle, lower-third, or callout, then edit the text on the preview."
    flush
  >
    <TitlePresetTiles oninsert={insertTitle} />
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
    <div
      in:fly={{ y: 6, duration: motionDuration(200), easing: cubicOut }}
      class="flex flex-col gap-3 border-t border-border/50 pt-3"
    >
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
        {#snippet action()}
          <div class="flex items-center gap-1">
            <Button
              variant="ghost"
              size="xs"
              class="text-[10px]"
              disabled={!startFromPlayhead}
              title="Move the start to the playhead"
              onclick={() => startFromPlayhead && updateSelected(startFromPlayhead, true)}
            >
              Start here
            </Button>
            <Button
              variant="ghost"
              size="xs"
              class="text-[10px]"
              disabled={!endFromPlayhead}
              title="Move the end to the playhead"
              onclick={() => endFromPlayhead && updateSelected(endFromPlayhead, true)}
            >
              End here
            </Button>
          </div>
        {/snippet}

        {#if outOfClip}
          <div
            class="flex items-center gap-2 rounded-lg border border-border/60 bg-card/60 px-2.5 py-1.5 text-[10px] text-muted-foreground"
          >
            <span class="flex-1 leading-snug">
              This annotation is timed outside the trimmed clip, so it never plays.
            </span>
            <Button variant="outline" size="xs" onclick={fitToClip}>Fit to clip</Button>
          </div>
        {/if}

        <!-- Bounded by the clip, not the raw recording: the old 0..duration range
             let you park an annotation outside the trim, which the export then
             silently moved back in. -->
        <SliderControl
          label="Start"
          value={a.start}
          min={clipIn}
          max={Math.max(a.end - 0.1, clipIn)}
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
          max={clipOut}
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
        <div class="flex justify-end">
          <Button variant="ghost" size="xs" class="text-[10px]" onclick={resetFades}>
            Reset fades
          </Button>
        </div>
      </PanelSection>

      <!-- Presets now lead here too: this section used to offer a raw bezier
           graph and nothing else. -->
      <PanelSection title="Fade curves" collapsible defaultOpen={false}>
        {#snippet action()}
          <Button variant="ghost" size="xs" class="text-[10px]" onclick={resetCurves}>
            Reset curves
          </Button>
        {/snippet}
        <EasingControl
          value={{ in: a.easeIn, out: a.easeOut }}
          onpick={(next) => {
            store.pushUndoState();
            updateSelected({ easeIn: { ...next }, easeOut: { ...next } });
          }}
          ondrag={(next, which) => {
            // Fires per pointermove; coalesce so a whole curve drag is one undo
            // entry, not one per frame.
            store.pushUndoStateCoalesced(`anno-curve-${a.id}-${which}`, 500);
            updateSelected(which === "out" ? { easeOut: next } : { easeIn: next });
          }}
          size={220}
        />
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
