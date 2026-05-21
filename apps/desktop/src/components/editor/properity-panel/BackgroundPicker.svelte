<script lang="ts">
  import LazyExternalImage from "$components/common/LazyExternalImage.svelte";
  import {
    COLOR_PRESETS,
    GRADIENT_PRESETS,
    MAX_FRAME_PADDING_PERCENT,
    WALLPAPERS,
    wallpaperBackgroundValue,
    type BackgroundType,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    Blend,
    FolderOpen,
    ImageIcon,
    LayoutTemplate,
    Move,
    Palette,
    Sparkles,
    Square,
    SquareRoundCorner,
  } from "@lucide/svelte";
  import {
    getRecentColors,
    pushRecentColor,
  } from "$lib/annotations/recent-colors";
  import { Button } from "@recast/ui/button";
  import { ColorField } from "@recast/ui/color-field";
  import { cn } from "@recast/ui/utils";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Image } from "@unpic/svelte";
  import { SliderControl } from "@recast/ui/slider-control";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  type BackgroundMode = {
    type: BackgroundType;
    label: string;
    icon: typeof Sparkles;
  };

  const backgroundModes: BackgroundMode[] = [
    { type: "wallpaper", label: "Wallpaper", icon: Sparkles },
    { type: "color", label: "Color", icon: Palette },
    { type: "gradient", label: "Gradient", icon: Blend },
    { type: "image", label: "Image", icon: ImageIcon },
  ];

  const DEFAULT_BACKGROUND_VALUES: Record<BackgroundType, string> = {
    wallpaper: WALLPAPERS[0] ? wallpaperBackgroundValue(WALLPAPERS[0].id) : "",
    color: COLOR_PRESETS[0] ?? "#000000",
    gradient:
      GRADIENT_PRESETS[0]?.value ??
      "linear-gradient(135deg, #111827 0%, #1f2937 100%)",
    image: "",
  };

  let { store }: Props = $props();

  let recents = $state<string[]>(getRecentColors());
  function rememberColor(color: string) {
    recents = pushRecentColor(color);
  }

  let blurValue = $state(0);
  let paddingValue = $state(0);
  let borderRadiusValue = $state(0);

  function isValidValueForType(type: BackgroundType, value: string) {
    switch (type) {
      case "wallpaper":
        return WALLPAPERS.some((w) => wallpaperBackgroundValue(w.id) === value);
      case "color":
        return /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
      case "gradient":
        return value.includes("gradient(");
      case "image":
        return value.length > 0;
      default:
        return false;
    }
  }

  function isValidImageValue(value: string) {
    if (!value) return false;
    // Explicitly reject non-image values that might linger in
    // `backgroundValue` after switching tabs (gradient strings, colour
    // hex, internal asset ids). Without this guard these slip through to
    // the `<Image>` element below, which feeds them into convertFileSrc
    // and triggers a Tauri asset-protocol "file does not exist" error.
    if (
      value.includes("gradient(") ||
      value.startsWith("#") ||
      value.startsWith("asset:")
    ) {
      return false;
    }
    return (
      value.startsWith("data:") ||
      value.startsWith("http://") ||
      value.startsWith("https://") ||
      value.startsWith("asset://") ||
      value.startsWith("/wallpapers/") ||
      value.endsWith(".png") ||
      value.endsWith(".jpg") ||
      value.endsWith(".jpeg") ||
      value.endsWith(".webp")
    );
  }

  function getSelectionValue(type: BackgroundType) {
    return isValidValueForType(type, store.backgroundValue)
      ? store.backgroundValue
      : DEFAULT_BACKGROUND_VALUES[type];
  }

  function applyBackground(
    type: BackgroundType,
    value = getSelectionValue(type),
  ) {
    // When the user clicks the "Image" tab and there is no valid image yet,
    // jump straight into the file picker instead of setting an empty value
    // (which would leave the preview showing the fallback dark background).
    if (type === "image" && !value) {
      void pickBackgroundImage();
      return;
    }
    store.setBackground({ type, value });
  }

  async function pickBackgroundImage() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose Background Image",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (!selected || typeof selected !== "string") return;
    store.setBackground({ type: "image", value: selected });
  }

  function getImagePreviewSrc(value: string) {
    if (!value) return "";
    // Mirror isValidImageValue's rejections — feeding a gradient string or
    // colour hex to convertFileSrc reaches Tauri's asset protocol and
    // produces "File does not exist" log spam.
    if (
      value.includes("gradient(") ||
      value.startsWith("#") ||
      value.startsWith("asset:")
    ) {
      return "";
    }
    if (
      value.startsWith("data:") ||
      value.startsWith("http://") ||
      value.startsWith("https://") ||
      value.startsWith("asset://") ||
      value.startsWith("/wallpapers/")
    ) {
      return value;
    }
    return convertFileSrc(value);
  }

  $effect(() => {
    blurValue = store.backgroundBlur;
    paddingValue = store.padding;
    borderRadiusValue = store.borderRadius;
  });
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <!-- Mode switcher: dense icon tabs instead of 2×2 cards -->
  <PanelSection
    title="Canvas"
    hint="Background styling and frame spacing are previewed live in the editor."
    flush
  >
    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 ring-1 ring-inset ring-border/40 p-0.5"
    >
      {#each backgroundModes as mode}
        {@const Icon = mode.icon}
        {@const isActive = store.backgroundType === mode.type}
        <Button
          variant="raw"
          size="xs"
          onclick={() => applyBackground(mode.type)}
          aria-pressed={isActive}
          title={mode.label}
          class={cn(
            "flex-1 gap-1",
            isActive
              ? "bg-card text-foreground shadow-(--shadow-craft-inset)"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon size={11} class={isActive ? "text-primary" : "text-muted-foreground"}/>
          <span class="hidden @[260px]/panel:inline">{mode.label}</span>
        </Button>
      {/each}
    </div>
  </PanelSection>

  {#if store.backgroundType === "wallpaper"}
    <PanelSection title="Wallpapers" flush>
      {#snippet action()}
        <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
          {WALLPAPERS.length}
        </span>
      {/snippet}
      <div class="grid grid-cols-3 gap-1.5">
        {#each WALLPAPERS as wallpaper (wallpaper.id)}
          {@const wallpaperValue = wallpaperBackgroundValue(wallpaper.id)}
          {@const isSelected = store.backgroundValue === wallpaperValue}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("wallpaper", wallpaperValue)}
            class={cn(
              "group relative aspect-video overflow-hidden rounded-md border transition-all",
              isSelected
                ? "border-primary ring-2 ring-primary/30"
                : "border-border hover:border-foreground/30",
            )}
            title={wallpaper.label}
            aria-label="Use {wallpaper.label} background"
            aria-pressed={isSelected}
          >
            <LazyExternalImage
              assetId={wallpaper.id}
              alt={wallpaper.label}
              tier="thumb"
              class="size-full object-cover transition-transform duration-200 group-hover:scale-[1.03]"
            />
          </Button>
        {/each}
      </div>
    </PanelSection>
  {:else if store.backgroundType === "color"}
    <PanelSection
      title="Color"
      hint="Solid backgrounds keep attention on the recording itself."
      flush
    >
      <div class="grid grid-cols-6 gap-1.5">
        {#each COLOR_PRESETS as color}
          {@const isSelected = store.backgroundValue === color}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("color", color)}
            aria-label="Use color {color}"
            aria-pressed={isSelected}
            class={cn(
              "aspect-square rounded-md border-2 transition-all",
              isSelected
                ? "border-foreground shadow-sm"
                : "border-border/40 hover:border-border",
            )}
            style="background-color: {color}"
          ></Button>
        {/each}
      </div>

      <div class="mt-2">
        <ColorField
          label="Custom"
          value={store.backgroundValue.startsWith("#")
            ? store.backgroundValue
            : DEFAULT_BACKGROUND_VALUES.color}
          {recents}
          oncommit={(c: string) => {
            store.pushUndoState();
            applyBackground("color", c);
            rememberColor(c);
          }}
        />
      </div>
    </PanelSection>
  {:else if store.backgroundType === "gradient"}
    <PanelSection
      title="Gradients"
      hint="Preset gradient backdrops render live in the preview."
      flush
    >
      <div class="grid grid-cols-2 gap-1.5">
        {#each GRADIENT_PRESETS as gradient}
          {@const isSelected = store.backgroundValue === gradient.value}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("gradient", gradient.value)}
            class={cn(
              "group relative h-16 overflow-hidden rounded-md border p-2 text-left transition-all",
              isSelected
                ? "border-primary ring-2 ring-primary/30"
                : "border-border hover:border-foreground/30",
            )}
            style="background: {gradient.value}"
            aria-label="Use {gradient.label} gradient"
            aria-pressed={isSelected}
          >
            <div class="flex h-full items-end">
              <span
                class="rounded border border-black/10 bg-black/40 px-1.5 py-0.5 text-[10px] font-medium text-white backdrop-blur-sm"
              >
                {gradient.label}
              </span>
            </div>
          </Button>
        {/each}
      </div>
    </PanelSection>
  {:else}
    <PanelSection
      title="Image"
      hint="Imported images fit to cover the full canvas."
      flush
    >
      {#snippet action()}
        <Button
          variant="outline"
          size="xs"
          class="gap-1.5"
          onclick={pickBackgroundImage}
        >
          <FolderOpen size={11} />
          {store.backgroundValue ? "Replace" : "Choose"}
        </Button>
      {/snippet}
      {#if store.backgroundValue && isValidImageValue(store.backgroundValue)}
        <div
          class="overflow-hidden rounded-md border border-border bg-background"
        >
          <Image
            src={getImagePreviewSrc(store.backgroundValue)}
            alt="Selected background"
            layout="constrained"
            width={320}
            aspectRatio={16 / 9}
            objectFit="cover"
            loading="lazy"
            decoding="async"
            class="max-h-56 w-full"
          />
        </div>
      {:else}
        <div
          class="flex h-20 items-center justify-center rounded-md border border-dashed border-border bg-muted/20 text-[11px] text-muted-foreground"
        >
          No image selected
        </div>
      {/if}
    </PanelSection>
  {/if}

  <!-- Finishing controls (always visible) -->
  <PanelSection
    title="Finishing"
    hint="Blur softens image-based backgrounds. Padding controls the space around the video frame as a percentage of frame size."
  >
    <SliderControl
      label="Background blur"
      bind:value={blurValue}
      min={0}
      max={100}
      step={1}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.backgroundBlur = next;
      }}
    >
      {#snippet icon()}
        <Blend size={11} />
      {/snippet}
    </SliderControl>

    <SliderControl
      label="Frame padding"
      bind:value={paddingValue}
      min={0}
      max={MAX_FRAME_PADDING_PERCENT}
      step={1}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.padding = next;
      }}
    >
      {#snippet icon()}
        <LayoutTemplate size={11} />
      {/snippet}
    </SliderControl>

    <SliderControl
      label="Corner radius"
      bind:value={borderRadiusValue}
      min={0}
      max={50}
      step={1}
      unit="%"
      onstart={() => store.pushUndoState()}
      onchange={(next) => {
        store.borderRadius = next;
      }}
    >
      {#snippet icon()}
        <SquareRoundCorner size={11} />
      {/snippet}
    </SliderControl>
  </PanelSection>

  <!-- Drop shadow — casts a soft shadow under the video rect onto the background. -->
  <PanelSection
    title="Drop shadow"
    hint="Adds depth by casting a soft shadow under the recording onto the canvas background."
    flush
  >
    {#snippet action()}
      <Button
        variant={store.shadow.enabled ? "default_soft" : "outline"}
        size="xs"
        class="gap-1.5"
        onclick={() => {
          store.pushUndoState();
          store.updateShadow({ enabled: !store.shadow.enabled });
        }}
        aria-pressed={store.shadow.enabled}
      >
        <Square size={11} />
        {store.shadow.enabled ? "On" : "Off"}
      </Button>
    {/snippet}

    {#if store.shadow.enabled}
      <div class="space-y-2.5">
        <SliderControl
          label="Blur"
          value={store.shadow.blur}
          min={0}
          max={100}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ blur: v })}
        >
          {#snippet icon()}
            <Blend size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Spread"
          value={store.shadow.spread}
          min={0}
          max={50}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ spread: v })}
        >
          {#snippet icon()}
            <SquareRoundCorner size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Offset Y"
          value={store.shadow.offsetY}
          min={-40}
          max={40}
          step={1}
          unit="px"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ offsetY: v })}
        >
          {#snippet icon()}
            <Move size={11} />
          {/snippet}
        </SliderControl>

        <SliderControl
          label="Opacity"
          value={store.shadow.opacity}
          min={0}
          max={100}
          step={1}
          unit="%"
          onstart={() => store.pushUndoState()}
          onchange={(v) => store.updateShadow({ opacity: v })}
        />

        <ColorField
          label="Shadow color"
          value={store.shadow.color || "#000000"}
          {recents}
          oncommit={(c: string) => {
            store.pushUndoState();
            store.updateShadow({ color: c });
            rememberColor(c);
          }}
        />
      </div>
    {/if}
  </PanelSection>
</div>
