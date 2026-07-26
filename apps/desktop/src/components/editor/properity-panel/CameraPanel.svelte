<script lang="ts">
  import {
    cameraPlacementFromPreset,
    cameraPresetFromPlacement,
    type CameraPositionPreset,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { VideoOff } from "@recast/icons";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { cn } from "@recast/ui/utils";
  import { SliderControl } from "@recast/ui/slider-control";
  import { dotStyleFor, labelFor } from "./camera-panel.logic";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
    /**
     * Path to the captured `camera.mp4` for this project, or null/empty
     * when the recording was made without a camera. Drives the empty-state
     * UI. The panel is always present in the tab strip but stays in
     * "no camera track" mode unless this resolves to a real file.
     */
    cameraPath: string | null | undefined;
  }

  let { store, cameraPath }: Props = $props();

  const hasCamera = $derived(!!cameraPath);

  // Video pixel aspect. The bubble is square in pixels, so its UV height is
  // `width * aspect`; the presets need it to anchor vertically on a wide frame.
  const videoAspect = $derived(
    store.metadata && store.metadata.height
      ? store.metadata.width / store.metadata.height
      : 1,
  );

  // Derived from the placement so a preview drag onto a corner re-highlights
  // the matching chip without a re-click.
  const activePreset = $derived(
    cameraPresetFromPlacement(store.cameraOverlay.defaultPlacement, videoAspect),
  );

  function applyPreset(preset: CameraPositionPreset) {
    if (preset === "custom") return; // Custom is the drag fallback.
    store.pushUndoState();
    const next = cameraPlacementFromPreset(
      preset,
      store.cameraOverlay.defaultPlacement.width,
      undefined,
      videoAspect,
    );
    store.updateCameraOverlay({ defaultPlacement: next });
  }

  function setSize(size: number) {
    // Anchor the resize on the current preset corner so the bubble doesn't
    // drift; custom placements just scale from their top-left.
    const current = store.cameraOverlay.defaultPlacement;
    if (activePreset === "custom") {
      store.updateCameraOverlay({
        defaultPlacement: {
          ...current,
          width: size,
          height: Math.min(1, size * videoAspect),
        },
      });
      return;
    }
    const next = cameraPlacementFromPreset(activePreset, size, undefined, videoAspect);
    store.updateCameraOverlay({ defaultPlacement: next });
  }

  // 3×3 grid mirroring the spatial position each chip represents, so users
  // pick by location rather than reading labels.
  const presetGrid: Array<CameraPositionPreset | null> = [
    "top-left", "top-center", "top-right",
    "left-center", null, "right-center",
    "bottom-left", "bottom-center", "bottom-right",
  ];

  const shapeOptions = [
    { id: "circle" as const, label: "Circle" },
    { id: "rounded" as const, label: "Rounded" },
    { id: "square" as const, label: "Square" },
  ];
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  {#if hasCamera}
    <div class="flex items-center justify-between gap-2 rounded-md border border-border/60 bg-card/40 px-2.5 py-1.5">
      <span class="text-[11px] text-muted-foreground">
        Composite the camera track onto the screen video.
      </span>
      <SegmentedToggle
        checked={store.cameraOverlay.enabled}
        offLabel="Hidden"
        onLabel="Visible"
        size="xs"
        aria-label="Camera visibility"
        onCheckedChange={(next) => {
          store.pushUndoState();
          store.updateCameraOverlay({ enabled: next });
        }}
      />
    </div>
  {/if}

  {#if !hasCamera}
    <!-- Empty state. The panel stays in the tab strip for a predictable
         layout, collapsed to an actionable hint. -->
    <div
      class="flex flex-col items-start gap-2 rounded-lg border border-dashed border-border/60 bg-muted/30 p-3"
    >
      <div
        class="flex size-7 items-center justify-center rounded-md bg-background/60 text-muted-foreground"
      >
        <VideoOff size={14} />
      </div>
      <p class="text-[11px] font-medium text-foreground">
        No camera track in this recording.
      </p>
      <p class="text-[10px] leading-snug text-muted-foreground">
        Enable the camera before starting your next recording to use this
        panel. Position, size, and shape can be tweaked here once a camera
        track is captured.
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
                  ? "border-primary/60 bg-primary/8 text-foreground"
                  : "border-transparent bg-background/40 text-foreground/80 hover:border-border hover:bg-background/80",
              )}
            >
              <span
                aria-hidden="true"
                class={cn(
                  "absolute size-1.5 rounded-full transition-colors duration-150",
                  isActive ? "bg-primary" : "bg-foreground/35 group-hover:bg-foreground/60",
                )}
                style={dotStyleFor(cell)}
              ></span>
            </button>
          {/if}
        {/each}
      </div>
    </PanelSection>

    <PanelSection
      title="Size"
      hint="Bubble width as a percentage of the frame, or drag its corners in the preview. Height matches width (1:1 only for now)."
    >
      <SliderControl
        label="Bubble size"
        value={Math.round(store.cameraOverlay.defaultPlacement.width * 100)}
        min={8}
        max={32}
        step={1}
        unit="%"
        onstart={() => store.pushUndoState()}
        onchange={(next) => setSize(next / 100)}
      />
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
                ? "border-primary/60 bg-primary/8 text-foreground"
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
      {/if}
    </PanelSection>
  {/if}
</div>
