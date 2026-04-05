<script lang="ts">
  import {
    COLOR_PRESETS,
    GRADIENT_PRESETS,
    WALLPAPERS,
    type BackgroundType,
    type EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { cn } from "$lib/utils";
  import {
    Blend,
    ImageIcon,
    LayoutTemplate,
    Palette,
    Sparkles,
  } from "@lucide/svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import InspectorHint from "./InspectorHint.svelte";
  import SliderControl from "./SliderControl.svelte";

  interface Props {
    store: EditorStore;
  }

  type BackgroundMode = {
    type: BackgroundType;
    label: string;
    hint: string;
    icon: typeof Sparkles;
    available?: boolean;
  };

  const backgroundModes: BackgroundMode[] = [
    {
      type: "wallpaper",
      label: "Wallpaper",
      hint: "Built-in image backdrops.",
      icon: Sparkles,
      available: true,
    },
    {
      type: "color",
      label: "Color",
      hint: "Solid fill canvas.",
      icon: Palette,
      available: true,
    },
    {
      type: "gradient",
      label: "Gradient",
      hint: "Preset gradients for more depth.",
      icon: Blend,
      available: true,
    },
    {
      type: "image",
      label: "Image",
      hint: "Custom uploaded backgrounds.",
      icon: ImageIcon,
      available: true,
    },
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

  function isValidValueForType(type: BackgroundType, value: string) {
    switch (type) {
      case "wallpaper":
        return WALLPAPERS.some((wallpaper) => wallpaper.src === value);
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

  function getSelectionValue(type: BackgroundType) {
    return isValidValueForType(type, store.backgroundValue)
      ? store.backgroundValue
      : DEFAULT_BACKGROUND_VALUES[type];
  }

  function applyBackground(
    type: BackgroundType,
    value = getSelectionValue(type),
  ) {
    const mode = backgroundModes.find((item) => item.type === type);
    if (!mode?.available) return;

    store.setBackground({
      type,
      value,
    });
  }

  async function pickBackgroundImage() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose Background Image",
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp"],
        },
      ],
    });

    if (!selected || typeof selected !== "string") return;
    applyBackground("image", selected);
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

  function getActiveBackgroundLabel() {
    if (store.backgroundType === "wallpaper") {
      return (
        WALLPAPERS.find((wallpaper) => wallpaper.src === store.backgroundValue)
          ?.label ?? "Wallpaper"
      );
    }

    if (store.backgroundType === "gradient") {
      return (
        GRADIENT_PRESETS.find(
          (gradient) => gradient.value === store.backgroundValue,
        )?.label ?? "Custom gradient"
      );
    }

    if (store.backgroundType === "color") {
      return store.backgroundValue.toUpperCase();
    }

    return "Image";
  }

  $effect(() => {
    blurValue = store.backgroundBlur;
    paddingValue = store.padding;
  });
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-300">
  <div class="flex items-start justify-between gap-3">
    <div>
      <div class="flex items-center gap-2">
        <h3 class="text-sm font-semibold text-foreground">Canvas</h3>
        <InspectorHint
          content="Background styling and frame spacing are previewed live in the editor."
        />
      </div>
    </div>
  </div>

  <div class="grid grid-cols-2 gap-2">
    {#each backgroundModes as mode}
      {@const Icon = mode.icon}
      <button
        type="button"
        onclick={() => applyBackground(mode.type)}
        disabled={!mode.available}
        aria-pressed={store.backgroundType === mode.type}
        title={mode.hint}
        class="group rounded-2xl border px-3 py-3 text-left transition-all duration-200 {store.backgroundType ===
        mode.type
          ? 'border-primary/60 bg-primary/10 shadow-[0_8px_30px_rgba(59,130,246,0.08)]'
          : 'border-border/70 bg-background/70 hover:border-border hover:bg-background'} {mode.available
          ? ''
          : 'cursor-not-allowed opacity-60'}"
      >
        <div class="flex items-start justify-between gap-2">
          <span
            class="flex h-8 w-8 items-center justify-center rounded-xl border border-border/70 bg-muted/70 text-muted-foreground transition-colors {store.backgroundType ===
            mode.type
              ? 'border-primary/20 bg-primary/10 text-primary'
              : 'group-hover:text-foreground'}"
          >
            <Icon size={14} />
          </span>
        </div>

        <p class="mt-2 text-xs font-semibold text-foreground">{mode.label}</p>
      </button>
    {/each}
  </div>

  {#if store.backgroundType === "wallpaper"}
    <section id="wallpaper-background">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h4 class="text-sm font-semibold text-foreground">Wallpapers</h4>
        <span class="text-[11px] font-medium text-muted-foreground">
          {WALLPAPERS.length} options
        </span>
      </div>

      <div class="grid grid-cols-3 gap-2 mx-auto">
        {#each WALLPAPERS as wallpaper}
          <button
            type="button"
            onclick={() => applyBackground("wallpaper", wallpaper.src)}
            class={cn(
              "group relative aspect-sq overflow-hidden rounded-lg h-20 border transition-all duration-200",
              {
                "border-primary shadow-[0_10px_24px_rgba(59,130,246,0.16)] ring-2 ring-primary/25":
                  store.backgroundValue === wallpaper.src,
                "border-border/60 hover:border-border hover:shadow-sm":
                  store.backgroundValue !== wallpaper.src,
              },
            )}
            title={wallpaper.label}
            aria-label={`Use ${wallpaper.label} background`}
          >
            <img
              src={wallpaper.src}
              alt={wallpaper.label}
              class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
              draggable="false"
            />
            <div
              class="absolute inset-x-0 bottom-0 bg-linear-to-t from-black/65 to-transparent px-2 pb-2 pt-4 text-[10px] font-medium text-white/90"
            >
              {wallpaper.label}
            </div>
          </button>
        {/each}
      </div>
    </section>
  {:else if store.backgroundType === "color"}
    <section id="color-background">
      <div class="mb-3 flex items-center gap-2">
        <h4 class="text-sm font-semibold text-foreground">Color Fill</h4>
        <InspectorHint
          content="Solid backgrounds keep attention on the recording itself."
        />
      </div>

      <div class="grid grid-cols-5 gap-2">
        {#each COLOR_PRESETS as color}
          <button
            type="button"
            onclick={() => applyBackground("color", color)}
            aria-label={`Use color ${color}`}
            class={cn(
              "aspect-square rounded-2xl border-2 transition-all duration-200",
              {
                "scale-105 border-foreground shadow-md":
                  store.backgroundValue === color,
                "border-border/30 hover:border-border hover:scale-[1.03]":
                  store.backgroundValue !== color,
              },
            )}
            style="background-color: {color}"
          ></button>
        {/each}
      </div>

      <div
        class="mt-4 rounded-2xl border border-border/70 bg-background/70 p-3"
      >
        <label class="flex items-center gap-3">
          <input
            type="color"
            value={store.backgroundValue.startsWith("#")
              ? store.backgroundValue
              : DEFAULT_BACKGROUND_VALUES.color}
            oninput={(event) =>
              applyBackground(
                "color",
                (event.currentTarget as HTMLInputElement).value,
              )}
            class="h-10 w-10 cursor-pointer rounded-xl border border-border bg-transparent"
            aria-label="Choose a custom background color"
          />
          <span class="min-w-0">
            <span class="block text-xs font-semibold text-foreground">
              Custom color
            </span>
            <span
              class="mt-1 block truncate font-mono text-[11px] text-muted-foreground"
            >
              {store.backgroundValue.toUpperCase()}
            </span>
          </span>
        </label>
      </div>
    </section>
  {:else if store.backgroundType === "gradient"}
    <section id="gradient-background">
      <div class="mb-3 flex items-center gap-2">
        <h4 class="text-sm font-semibold text-foreground">Gradients</h4>
        <InspectorHint
          content="These presets render in the preview using their selected gradient values."
        />
      </div>

      <div class="grid grid-cols-2 gap-2">
        {#each GRADIENT_PRESETS as gradient}
          <button
            type="button"
            onclick={() => applyBackground("gradient", gradient.value)}
            class="group h-20 rounded-2xl border p-3 text-left transition-all duration-200 {store.backgroundValue ===
            gradient.value
              ? 'border-primary shadow-[0_10px_24px_rgba(59,130,246,0.16)] ring-2 ring-primary/25'
              : 'border-border/60 hover:border-border hover:shadow-sm'}"
            style="background: {gradient.value}"
            aria-label={`Use ${gradient.label} gradient`}
          >
            <div class="flex h-full items-end">
              <span
                class="rounded-full bg-black/35 px-2 py-1 text-[11px] font-semibold text-white shadow-sm backdrop-blur-sm"
              >
                {gradient.label}
              </span>
            </div>
          </button>
        {/each}
      </div>
    </section>
  {:else}
    <section id="image-background">
      <div class="flex items-center justify-between gap-3">
        <div class="flex items-center gap-2">
          <h4 class="text-sm font-semibold text-foreground">
            Image Background
          </h4>
          <InspectorHint
            content="Imported images are fit to cover the full canvas in the editor preview."
          />
        </div>
        <button
          type="button"
          onclick={pickBackgroundImage}
          class="rounded-2xl border border-border/70 bg-background/80 px-3 py-2 text-xs font-semibold text-foreground transition-colors hover:border-border hover:bg-background"
        >
          {store.backgroundValue ? "Replace" : "Choose"}
        </button>
      </div>

      {#if store.backgroundValue}
        <div
          class="mt-3 overflow-hidden rounded-2xl border border-border/70 bg-background/80"
        >
          <img
            src={getImagePreviewSrc(store.backgroundValue)}
            alt="Selected background"
            class="h-30 w-full object-cover"
          />
        </div>
      {:else}
        <div
          class="mt-3 flex h-30 items-center justify-center rounded-2xl border border-dashed border-border/70 bg-background/60 text-xs text-muted-foreground"
        >
          Choose an image
        </div>
      {/if}
    </section>
  {/if}

  <section id="finishing">
    <div class="mb-3 flex items-center gap-2">
      <h4 class="text-sm font-semibold text-foreground">Finishing</h4>
      <InspectorHint
        content="Blur softens image-based backgrounds. Padding controls the space around the video frame."
      />
    </div>

    <div class="space-y-3">
      <SliderControl
        label="Background blur"
        bind:value={blurValue}
        min={0}
        max={100}
        step={1}
        unit="%"
        onstart={() => store.pushUndoState()}
        onchange={(nextValue) => {
          store.backgroundBlur = nextValue;
        }}
      >
        {#snippet icon()}
          <Blend size={12} />
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
        onchange={(nextValue) => {
          store.padding = nextValue;
        }}
      >
        {#snippet icon()}
          <LayoutTemplate size={12} />
        {/snippet}
      </SliderControl>
    </div>
  </section>
</div>
