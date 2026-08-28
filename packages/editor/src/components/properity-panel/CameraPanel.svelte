<script lang="ts">
import { Ruler, VideoOff } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { SegmentedToggle } from "@recast/ui/segmented";
import { SliderControl } from "@recast/ui/slider-control";
import { cn } from "@recast/ui/utils";
import type { CameraCapture } from "../../lib/wire-types";
import {
	type CameraPositionPreset,
	cameraPlacementFromPreset,
	cameraPresetFromPlacement,
	type EditorStore,
} from "../../stores/editor-store.svelte";
import { cameraPlacementAt } from "../_components/camera-overlay.logic";
import { cameraAvailability, dotStyleFor, labelFor } from "./camera-panel.logic";
import { clampValue } from "./draggable-value.logic";
import EasingControl from "./EasingControl.svelte";
import NumberField from "./NumberField.svelte";
import PanelSection from "./PanelSection.svelte";
import PropRow from "./PropRow.svelte";
import Stepper from "./Stepper.svelte";

interface Props {
	store: EditorStore;
	/**
	 * Path to the captured `camera.mp4` for this project, or null/empty
	 * when the recording was made without a camera. Drives the empty-state
	 * UI. The panel is always present in the tab strip but stays in
	 * "no camera track" mode unless this resolves to a real file.
	 */
	cameraPath: string | null | undefined;
	/** Why that path is or isn't set. The path alone can't distinguish a camera
	 *  that was switched off from a project recorded before camera capture. */
	cameraCapture?: CameraCapture;
}

let { store, cameraPath, cameraCapture = "legacy" }: Props = $props();

const hasCamera = $derived(!!cameraPath);
const availability = $derived(cameraAvailability(cameraCapture, hasCamera));

// Video pixel aspect. The bubble is square in pixels, so its UV height is
// `width * aspect`; the presets need it to anchor vertically on a wide frame.
const videoAspect = $derived(
	store.metadata && store.metadata.height ? store.metadata.width / store.metadata.height : 1,
);

const perCut = $derived(store.cameraOverlay.keyframes.length > 0);

// The placement being edited: in per-cut mode it's the glide value at the
// playhead (the keyframe you're setting); else the static placement.
const currentBase = $derived(
	cameraPlacementAt(
		store.cameraOverlay.defaultPlacement,
		store.cameraOverlay.keyframes,
		store.currentTime,
		store.cameraOverlay.keyframeEasing,
	),
);

// Derived from the placement so a preview drag onto a corner re-highlights
// the matching chip without a re-click.
const activePreset = $derived(cameraPresetFromPlacement(currentBase, videoAspect));

function applyPreset(preset: CameraPositionPreset) {
	if (preset === "custom") return; // Custom is the drag fallback.
	store.pushUndoState();
	const next = cameraPlacementFromPreset(preset, currentBase.width, undefined, videoAspect);
	store.setCameraPlacement(next);
}

function setSize(size: number) {
	// Anchor the resize on the current preset corner so the bubble doesn't
	// drift; custom placements just scale from their top-left.
	if (activePreset === "custom") {
		store.setCameraPlacement({
			...currentBase,
			width: size,
			height: Math.min(1, size * videoAspect),
		});
		return;
	}
	store.setCameraPlacement(cameraPlacementFromPreset(activePreset, size, undefined, videoAspect));
}

// 3×3 grid mirroring the spatial position each chip represents, so users
// pick by location rather than reading labels.
const presetGrid: Array<CameraPositionPreset | null> = [
	"top-left",
	"top-center",
	"top-right",
	"left-center",
	null,
	"right-center",
	"bottom-left",
	"bottom-center",
	"bottom-right",
];

const shapeOptions = [
	{ id: "circle" as const, label: "Circle" },
	{ id: "rounded" as const, label: "Rounded" },
	{ id: "square" as const, label: "Square" },
];
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  {#if hasCamera}
    <div class="flex items-center justify-between gap-2">
      <span class="text-[11px] font-medium text-foreground">Show camera</span>
      <SegmentedToggle
        checked={store.cameraOverlay.enabled}
        size="xs"
        aria-label="Show camera"
        onCheckedChange={(next) => {
          store.pushUndoState();
          store.updateCameraOverlay({ enabled: next });
        }}
      />
    </div>
  {/if}

  {#if !hasCamera}
    <!-- Empty state. The panel stays in the tab strip for a predictable
         layout, collapsed to a hint that says which case this is: a camera
         that was off, a project older than camera capture, or a recorded
         track whose file has gone missing. All three used to read the same,
         so an old recording told you to enable a toggle that hadn't shipped
         when it was made. -->
    <div
      class="flex flex-col items-start gap-2 rounded-lg border border-dashed border-border/60 bg-muted/30 p-3"
    >
      <div
        class="flex size-7 items-center justify-center rounded-md bg-background/60 text-muted-foreground"
      >
        <VideoOff size={14} />
      </div>
      <p class="text-[11px] font-medium text-foreground">{availability.title}</p>
      <p class="text-[10px] leading-snug text-muted-foreground">
        {availability.description}
      </p>
    </div>
  {:else if store.cameraOverlay.enabled}
    <PanelSection
      title="Position"
      hint="Pick a corner or edge anchor. Drag the bubble in the preview for a custom position."
      flush
    >
      {#snippet action()}
        <span class="font-mono text-[10px] tracking-tight text-foreground/80">
          {activePreset === "custom" ? "Custom" : labelFor(activePreset)}
        </span>
      {/snippet}
      <div
        class="grid grid-cols-3 gap-1 rounded-lg border border-border/60 bg-muted/30 p-1 shadow-(--shadow-craft-inset)"
      >
        {#each presetGrid as cell, i (i)}
          {#if cell === null}
            <!-- Centre cell left empty so the chips map to bubble position. -->
            <div aria-hidden="true" class="aspect-square"></div>
          {:else}
            {@const isActive = activePreset === cell}
            <button
              type="button"
              aria-pressed={isActive}
              aria-label={labelFor(cell)}
              title={labelFor(cell)}
              onclick={() => applyPreset(cell)}
              class={cn(
                "group relative aspect-square overflow-hidden rounded-md border transition-all duration-150",
                "focus:outline-none focus:ring-2 focus:ring-ring/40",
                isActive
                  ? "border-foreground/40 bg-foreground/10 text-foreground"
                  : "border-transparent bg-background/40 text-foreground/80 hover:border-border hover:bg-background/80",
              )}
            >
              <span
                aria-hidden="true"
                class={cn(
                  "absolute size-1.5 rounded-full transition-colors duration-150",
                  isActive ? "bg-foreground" : "bg-foreground/35 group-hover:bg-foreground/60",
                )}
                style={dotStyleFor(cell)}
              ></span>
            </button>
          {/if}
        {/each}
      </div>
    </PanelSection>

    <PanelSection
      title="Per-cut position"
      hint="Give each cut its own camera position; the bubble glides between them. Scrub to a cut, then pick a preset or drag the bubble."
      flush
    >
      {#snippet action()}
        <SegmentedToggle
          checked={perCut}
          size="xs"
          aria-label="Per-cut camera position"
          onCheckedChange={(next) => store.setCameraPerCut(next)}
        />
      {/snippet}
      {#if perCut}
        <div class="flex items-center justify-between gap-2 pt-1">
          <span class="text-[10px] text-muted-foreground">
            {store.cameraOverlay.keyframes.length}
            {store.cameraOverlay.keyframes.length === 1 ? "position" : "positions"} · glides between cuts
          </span>
          <Button
            variant="ghost"
            size="xs"
            class="text-[10.5px] text-muted-foreground"
            onclick={() => store.removeCameraKeyframeNear(store.currentTime)}
          >
            Clear this cut
          </Button>
        </div>
      {/if}
    </PanelSection>

    <PanelSection
      title="Size"
      hint="Bubble width as a percentage of the frame, or drag its corners in the preview. Height matches width (1:1 only for now)."
      flush
    >
      {@const sizePct = Math.round(currentBase.width * 100)}
      <PropRow label="Width">
        <NumberField
          class="flex-1"
          label="Bubble size"
          icon={Ruler}
          value={sizePct}
          min={8}
          max={32}
          step={1}
          suffix="%"
          onDragStart={() => store.pushUndoState()}
          onInput={(v) => setSize(v / 100)}
          onCommit={(v, viaDrag) => {
            if (!viaDrag) store.pushUndoState();
            setSize(v / 100);
          }}
        />
        <Stepper
          label="width"
          onStep={(d) => {
            store.pushUndoState();
            setSize(clampValue(sizePct + d, 8, 32) / 100);
          }}
        />
      </PropRow>
    </PanelSection>

    <PanelSection
      title="Shape"
      hint="Circle for talking-head puck, rounded for app-style overlay, square for a sharp cut."
      flush
    >
      <div class="grid grid-cols-3 gap-1 rounded-lg border border-border/60 bg-muted/30 p-1 shadow-(--shadow-craft-inset)">
        {#each shapeOptions as opt (opt.id)}
          {@const isActive = store.cameraOverlay.shape === opt.id}
          <button
            type="button"
            aria-pressed={isActive}
            onclick={() => {
              store.pushUndoState();
              store.updateCameraOverlay({ shape: opt.id });
            }}
            class={cn(
              "rounded-md border px-2 py-1.5 text-[11px] font-medium transition-all duration-150",
              "focus:outline-none focus:ring-2 focus:ring-ring/40",
              isActive
                ? "border-foreground/40 bg-foreground/10 text-foreground"
                : "border-transparent bg-background/40 text-foreground/80 hover:border-border hover:bg-background/80",
            )}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    </PanelSection>

    <PanelSection
      title="Mirror"
      hint="On: the bubble matches a webcam preview. Off: it shows you as others see you, so text behind you reads correctly."
      flush
    >
      {#snippet action()}
        <SegmentedToggle
          checked={store.cameraOverlay.mirror}
          size="xs"
          aria-label="Mirror camera"
          onCheckedChange={(next) => {
            store.pushUndoState();
            store.updateCameraOverlay({ mirror: next });
          }}
        />
      {/snippet}
    </PanelSection>

    <PanelSection
      title="Grow on zoom"
      hint="When a zoom/focus region ramps in, the camera grows and drifts away from the focus so it never covers the zoomed area."
      flush
    >
      {#snippet action()}
        <SegmentedToggle
          checked={store.cameraOverlay.zoomFollow}
          size="xs"
          aria-label="Grow camera on zoom"
          onCheckedChange={(next) => {
            store.pushUndoState();
            store.updateCameraOverlay({ zoomFollow: next });
          }}
        />
      {/snippet}
      {#if store.cameraOverlay.zoomFollow}
        <SliderControl
          label="Strength"
          value={Math.round(store.cameraOverlay.zoomFollowStrength * 100)}
          min={0}
          max={100}
          step={5}
          unit="%"
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCameraOverlay({ zoomFollowStrength: next / 100 })}
        />
        <SliderControl
          label="Transition"
          value={Math.round(store.cameraOverlay.zoomFollowDuration * 1000)}
          min={200}
          max={1200}
          step={50}
          unit="ms"
          onstart={() => store.pushUndoState()}
          onchange={(next) => store.updateCameraOverlay({ zoomFollowDuration: next / 1000 })}
        />
        <EasingControl
          value={store.cameraOverlay.zoomFollowEasing}
          size={220}
          onpick={(v) => {
            store.pushUndoState();
            store.updateCameraOverlay({ zoomFollowEasing: v });
          }}
          ondrag={(v) =>
            store.updateCameraOverlayLive(
              { zoomFollowEasing: v },
              "camera-zoomfollow-easing",
            )}
        />
      {/if}
    </PanelSection>

    <PanelSection title="Shadow" hint="Drop shadow cast by the bubble. 0% turns it off.">
      <SliderControl
        label="Shadow"
        value={Math.round(store.cameraOverlay.shadow * 100)}
        min={0}
        max={100}
        step={5}
        unit="%"
        onstart={() => store.pushUndoState()}
        onchange={(next) => store.updateCameraOverlay({ shadow: next / 100 })}
      />
    </PanelSection>

    {#if perCut}
      <PanelSection
        title="Animation smoothness"
        hint="How the camera eases as it glides between per-cut positions."
      >
        <EasingControl
          value={store.cameraOverlay.keyframeEasing}
          size={220}
          onpick={(v) => {
            store.pushUndoState();
            store.updateCameraOverlay({ keyframeEasing: v });
          }}
          ondrag={(v) =>
            store.updateCameraOverlayLive(
              { keyframeEasing: v },
              "camera-keyframe-easing",
            )}
        />
      </PanelSection>
    {/if}
  {/if}
</div>
