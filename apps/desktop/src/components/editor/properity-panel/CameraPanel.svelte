<script lang="ts">
  import {
    cameraPlacementFromPreset,
    cameraPresetFromPlacement,
    type CameraPositionPreset,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    Eye,
    EyeOff,
    FlipHorizontal,
    VideoOff,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import SliderControl from "../_components/SliderControl.svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
    /**
     * Path to the captured `camera.mp4` for this project, or null/empty
     * when the recording was made without a camera. Drives the empty-state
     * UI — the panel is always present in the tab strip but stays in
     * "no camera track" mode unless this resolves to a real file.
     */
    cameraPath: string | null | undefined;
  }

  let { store, cameraPath }: Props = $props();

  const hasCamera = $derived(!!cameraPath);

  // Active preset chip — derives from the placement on every change so a
  // drag in the preview that lands exactly on a corner re-highlights the
  // matching chip without the user re-clicking it.
  const activePreset = $derived(
    cameraPresetFromPlacement(store.cameraOverlay.defaultPlacement),
  );

  function applyPreset(preset: CameraPositionPreset) {
    if (preset === "custom") return; // Custom is the drag fallback.
    store.pushUndoState();
    const next = cameraPlacementFromPreset(
      preset,
      store.cameraOverlay.defaultPlacement.width,
    );
    store.updateCameraOverlay({ defaultPlacement: next });
  }

  function setSize(size: number) {
    // Resize anchored on the current preset corner so the bubble doesn't
    // visually drift when scaling. Falls back to keeping x/y as-is for
    // custom placements (pure scale-from-top-left, which is fine because
    // the user just dragged to that exact spot).
    const current = store.cameraOverlay.defaultPlacement;
    if (activePreset === "custom") {
      store.updateCameraOverlay({
        defaultPlacement: { ...current, width: size, height: size },
      });
      return;
    }
    const next = cameraPlacementFromPreset(activePreset, size);
    store.updateCameraOverlay({ defaultPlacement: next });
  }

  /** Human label for a preset id. Re-used in chip's aria-label/title and
   *  the panel header readout. */
  function labelFor(preset: CameraPositionPreset): string {
    return preset
      .split("-")
      .map((part) => part[0].toUpperCase() + part.slice(1))
      .join(" ");
  }

  /** Position the dot inside a preset chip so the chip looks like a
   *  miniature frame with the bubble placed where it will land. */
  function dotStyleFor(preset: CameraPositionPreset): string {
    if (preset === "custom") return "left:50%;top:50%;transform:translate(-50%,-50%);";
    const [row, col] = preset.split("-");
    let xPart = "";
    let yPart = "";
    let translateX = "";
    let translateY = "";
    if (col === "left") xPart = "left:18%;";
    else if (col === "right") xPart = "right:18%;";
    else { xPart = "left:50%;"; translateX = "translateX(-50%)"; }
    if (row === "top") yPart = "top:18%;";
    else if (row === "bottom") yPart = "bottom:18%;";
    else { yPart = "top:50%;"; translateY = "translateY(-50%)"; }
    const transform =
      translateX && translateY
        ? `transform:${translateX} ${translateY};`
        : translateX || translateY
          ? `transform:${translateX || translateY};`
          : "";
    return xPart + yPart + transform;
  }

  // Preset chip rows — laid out on a 3×3 grid that mirrors the spatial
  // position the chip represents (top-left chip is in the top-left grid
  // cell, etc.) so users can pick by spatial intuition rather than reading
  // each label.
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
    <!-- Visibility toggle bar (no section title — panel name is in header) -->
    <div class="flex items-center justify-between gap-2 rounded-md border border-border/60 bg-card/40 px-2.5 py-1.5">
      <span class="text-[11px] text-muted-foreground">
        Composite the camera track onto the screen video.
      </span>
      <Button
        variant={store.cameraOverlay.enabled ? "default_soft" : "outline"}
        size="xs"
        class="gap-1.5"
        onclick={() => {
          store.pushUndoState();
          store.updateCameraOverlay({
            enabled: !store.cameraOverlay.enabled,
          });
        }}
        aria-pressed={store.cameraOverlay.enabled}
      >
        {#if store.cameraOverlay.enabled}
          <Eye size={11} />
          Visible
        {:else}
          <EyeOff size={11} />
          Hidden
        {/if}
      </Button>
    </div>
  {/if}

  {#if !hasCamera}
    <!-- Empty state — panel still appears in the tab strip so the layout is
         predictable across recordings, but the controls collapse to a hint
         the user can act on next time they record. -->
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
    <!-- Position presets: 3×3 grid mirroring spatial position. -->
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
            <!-- Centre cell of the 3×3 — left empty so the corner/edge
                 chips visually map to where the bubble will sit on the
                 frame. The grid IS the legend. -->
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
              <!-- Tiny dot inside the chip showing where the bubble lands. -->
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
      hint="Bubble width as a percentage of the frame. Height matches width — Phase 1 ships 1:1 only."
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
      hint="On (default): the bubble matches what you see in a webcam preview. Off: the bubble shows you as others see you — text behind you reads correctly."
      flush
    >
      {#snippet action()}
        <Button
          variant={store.cameraOverlay.mirror ? "default_soft" : "outline"}
          size="xs"
          class="gap-1.5"
          onclick={() => {
            store.pushUndoState();
            store.updateCameraOverlay({
              mirror: !store.cameraOverlay.mirror,
            });
          }}
          aria-pressed={store.cameraOverlay.mirror}
        >
          <FlipHorizontal size={11} />
          {store.cameraOverlay.mirror ? "On" : "Off"}
        </Button>
      {/snippet}
    </PanelSection>
  {/if}
</div>
