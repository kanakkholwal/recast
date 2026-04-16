<script lang="ts">
  import {
    COLOR_PRESETS,
    GRADIENT_PRESETS,
    WALLPAPERS,
    type BackgroundType,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import {
    Blend,
    FolderOpen,
    ImageIcon,
    LayoutTemplate,
    Palette,
    Sparkles,
    SquareRoundCorner,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Image } from "@unpic/svelte";
  import InspectorHint from "./InspectorHint.svelte";
  import SliderControl from "./SliderControl.svelte";

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
    wallpaper: WALLPAPERS[0]?.src ?? "",
    color: COLOR_PRESETS[0] ?? "#000000",
    gradient:
      GRADIENT_PRESETS[0]?.value ??
      "linear-gradient(135deg, #111827 0%, #1f2937 100%)",
    image: "",
  };

  let { store }: Props = $props();

  let blurValue = $state(0);
  let paddingValue = $state(0);
  let borderRadiusValue = $state(0);

  function isValidValueForType(type: BackgroundType, value: string) {
    switch (type) {
      case "wallpaper":
        return WALLPAPERS.some((w) => w.src === value);
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

  function applyBackground(type: BackgroundType, value = getSelectionValue(type)) {
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

<div class="flex flex-col gap-5 animate-in fade-in duration-200">
  <!-- Mode switcher: dense icon tabs instead of 2×2 cards -->
  <section>
    <header class="mb-2 flex items-center gap-1.5">
      <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Canvas
      </h3>
      <InspectorHint content="Background styling and frame spacing are previewed live in the editor." />
    </header>
    <div class="flex items-center gap-0.5 rounded-md border border-border bg-muted/30 p-0.5">
      {#each backgroundModes as mode}
        {@const Icon = mode.icon}
        {@const isActive = store.backgroundType === mode.type}
        <Button
          variant={isActive ? "default_soft" : "ghost"}
          size="xs"
          class="flex-1 gap-1"
          onclick={() => applyBackground(mode.type)}
          aria-pressed={isActive}
          title={mode.label}
        >
          <Icon size={11} />
          <span class="hidden @[260px]/panel:inline">{mode.label}</span>
        </Button>
      {/each}
    </div>
  </section>

  {#if store.backgroundType === "wallpaper"}
    <section>
      <header class="mb-2 flex items-center justify-between gap-2">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Wallpapers
        </h3>
        <span class="text-[10px] font-mono tabular-nums text-muted-foreground">
          {WALLPAPERS.length}
        </span>
      </header>
      <div class="grid grid-cols-3 gap-1.5">
        {#each WALLPAPERS as wallpaper (wallpaper.src)}
          {@const isSelected = store.backgroundValue === wallpaper.src}
          <Button
            variant="raw"
            size="raw"
            onclick={() => applyBackground("wallpaper", wallpaper.src)}
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
            <Image
              src={wallpaper.thumb}
              alt={wallpaper.label}
              layout="constrained"
              width={160}
              aspectRatio={16 / 9}
              objectFit="cover"
              loading="lazy"
              decoding="async"
              class="size-full transition-transform duration-200 group-hover:scale-[1.03]"
            />
          </Button>
        {/each}
      </div>
    </section>
  {:else if store.backgroundType === "color"}
    <section>
      <header class="mb-2 flex items-center gap-1.5">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Color Fill
        </h3>
        <InspectorHint content="Solid backgrounds keep attention on the recording itself." />
      </header>
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

      <div class="mt-3 flex items-center gap-2 rounded-md border border-border bg-background px-2 py-2">
        <input
          type="color"
          value={store.backgroundValue.startsWith("#")
            ? store.backgroundValue
            : DEFAULT_BACKGROUND_VALUES.color}
          oninput={(e) => applyBackground("color", (e.currentTarget as HTMLInputElement).value)}
          class="size-7 shrink-0 cursor-pointer rounded border border-input bg-transparent"
          aria-label="Choose a custom background color"
        />
        <div class="min-w-0 flex-1">
          <p class="text-[11px] font-medium text-foreground">Custom color</p>
          <p class="truncate font-mono text-[10px] text-muted-foreground">
            {store.backgroundValue.toUpperCase()}
          </p>
        </div>
      </div>
    </section>
  {:else if store.backgroundType === "gradient"}
    <section>
      <header class="mb-2 flex items-center gap-1.5">
        <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Gradients
        </h3>
        <InspectorHint content="Preset gradient backdrops render live in the preview." />
      </header>
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
    </section>
  {:else}
    <section>
      <header class="mb-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5">
          <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Image Background
          </h3>
          <InspectorHint content="Imported images fit to cover the full canvas in the editor preview." />
        </div>
        <Button variant="outline" size="xs" class="gap-1.5" onclick={pickBackgroundImage}>
          <FolderOpen size={11} />
          {store.backgroundValue ? "Replace" : "Choose"}
        </Button>
      </header>

      {#if store.backgroundValue && isValidImageValue(store.backgroundValue)}
        <div class="overflow-hidden rounded-md border border-border bg-background">
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
    </section>
  {/if}

  <!-- Finishing controls (always visible) -->
  <section>
    <header class="mb-2 flex items-center gap-1.5">
      <h3 class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Finishing
      </h3>
      <InspectorHint content="Blur softens image-based backgrounds. Padding controls the space around the video frame." />
    </header>

    <div class="space-y-2.5">
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
        max={100}
        step={1}
        unit="px"
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
    </div>
  </section>
</div>
