<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface BackgroundControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { ColorPicker } from "@recast/ui/color-picker";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { ImageUp, Sparkles, X } from "@recast/icons";
  import {
    GRADIENT_PRESETS,
    MAGIC_PRESETS,
    MESH_PRESETS,
    PATTERN_PRESETS,
    SOLID_PRESETS,
  } from "../presets";
  import { imageFromFile } from "../image-input";
  import { IMAGE_BACKGROUND_CATEGORIES, imageBackgroundCss } from "../image-backgrounds";
  import type { BackgroundPreset } from "../types";
  import type { ImageBackground } from "../image-backgrounds";

  let { editor }: BackgroundControlProps = $props();

  let bgFileInput = $state<HTMLInputElement | null>(null);
  let lastColor = $state("#7dd4ad");

  const gradients = [...GRADIENT_PRESETS, ...MESH_PRESETS];

  // Random magic gradient (the reference's Shuffle affordance).
  function shuffleMagic() {
    const pool = MAGIC_PRESETS.filter((p) => p.id !== editor.backgroundId);
    const next = pool[Math.floor(Math.random() * pool.length)] ?? MAGIC_PRESETS[0];
    pick(next);
  }

  // Which of the three custom-background tiles reads as active. A preset swatch
  // selection leaves all three inactive.
  const customType = $derived.by(() => {
    if (editor.background.kind === "transparent") return "transparent";
    if (editor.backgroundId === "image") return "image";
    if (editor.backgroundId === "custom") return "color";
    return null;
  });

  const isImage = $derived(editor.backgroundId === "image");
  const imageCss = $derived(
    editor.background.kind === "gradient" ? editor.background.css : "",
  );
  const currentColor = $derived(
    editor.background.kind === "solid" ? editor.background.color : lastColor,
  );

  function pick(preset: BackgroundPreset) {
    editor.setBackground(preset.id, preset.background);
  }

  function pickImage(img: ImageBackground) {
    editor.setBackground(img.id, { kind: "gradient", css: imageBackgroundCss(img.url) });
  }

  function chooseColor(next: string) {
    lastColor = next;
    editor.setCustomColor(next);
  }

  async function onBgFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    const img = await imageFromFile(file);
    editor.setBackground("image", {
      kind: "gradient",
      css: `url("${img.src}") center / cover no-repeat`,
    });
  }

  const tileClass = (active: boolean) =>
    cn(
      "flex flex-col items-center justify-center gap-1.5 rounded-xl border py-2.5 transition-colors",
      active
        ? "border-primary bg-primary/5 ring-primary/20 ring-1"
        : "border-border bg-muted/30 hover:bg-accent hover:border-border",
    );

  const swatchClass = (active: boolean) =>
    cn(
      "border-border aspect-square border transition-transform hover:scale-105",
      active ? "ring-primary rounded-full ring-2 ring-offset-1" : "rounded-lg",
    );
</script>

<input bind:this={bgFileInput} type="file" accept="image/*" class="hidden" onchange={onBgFile} />

<!-- Custom Background: Image / Color / Transparent (clone's 3-tile grid). -->
<PanelSection title="Custom Background" collapsible defaultOpen>
  <div class="grid grid-cols-3 gap-2">
    <button type="button" class={tileClass(customType === "image")} onclick={() => bgFileInput?.click()}>
      <span
        class={cn(
          "flex size-7 items-center justify-center rounded-lg",
          customType === "image" ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground",
        )}
      >
        <ImageUp class="size-3.5" />
      </span>
      <span
        class={cn(
          "text-xs font-medium",
          customType === "image" ? "text-foreground" : "text-muted-foreground",
        )}
      >
        Image
      </span>
    </button>

    <button
      type="button"
      class={tileClass(customType === "color")}
      onclick={() => chooseColor(currentColor)}
    >
      <span class="border-border size-7 rounded-lg border" style:background={currentColor}></span>
      <span
        class={cn(
          "text-xs font-medium",
          customType === "color" ? "text-foreground" : "text-muted-foreground",
        )}
      >
        Color
      </span>
    </button>

    <button
      type="button"
      class={tileClass(customType === "transparent")}
      onclick={() => editor.setBackground("transparent", { kind: "transparent" })}
    >
      <span
        class={cn(
          "flex size-7 items-center justify-center rounded-lg",
          customType === "transparent" ? "bg-primary/10" : "bg-muted",
        )}
      >
        <span
          class="border-border size-3.5 rounded-full border"
          style="background:repeating-conic-gradient(#808080 0% 25%, #fff 0% 50%) 50% / 6px 6px;"
        ></span>
      </span>
      <span
        class={cn(
          "text-xs font-medium",
          customType === "transparent" ? "text-foreground" : "text-muted-foreground",
        )}
      >
        Transparent
      </span>
    </button>
  </div>

  {#if customType === "color"}
    <ColorPicker value={currentColor} oncommit={chooseColor} allowAlpha={false} />
  {/if}

  {#if isImage}
    <div class="border-border bg-muted/50 relative aspect-video overflow-hidden rounded-lg">
      <div class="size-full" style:background={imageCss}></div>
      <button
        type="button"
        class="bg-background/60 text-foreground hover:bg-destructive hover:text-destructive-foreground absolute right-2 top-2 rounded-md p-1 transition-colors"
        aria-label="Remove background image"
        onclick={() => pick(gradients[0])}
      >
        <X class="size-3.5" />
      </button>
    </div>
  {/if}
</PanelSection>

<!-- Gradients (classic + mesh). -->
<PanelSection title="Gradients" collapsible defaultOpen>
  <div class="grid grid-cols-6 gap-2">
    {#each gradients as preset (preset.id)}
      <button
        type="button"
        class={swatchClass(editor.backgroundId === preset.id)}
        style:background={preset.swatch}
        title={preset.label}
        aria-label={preset.label}
        aria-pressed={editor.backgroundId === preset.id}
        onclick={() => pick(preset)}
      ></button>
    {/each}
  </div>
</PanelSection>

<!-- Magic gradients (100): dark radial/conic/pattern glows. Collapsed by default
     so the heavy multi-layer swatches don't paint until the section is opened. -->
<PanelSection title="Magic" collapsible defaultOpen={false}>
  {#snippet action()}
    <Button variant="ghost" size="xs" onclick={shuffleMagic}>
      <Sparkles class="size-3.5" />
      Shuffle
    </Button>
  {/snippet}
  <div class="grid grid-cols-6 gap-2">
    {#each MAGIC_PRESETS as preset (preset.id)}
      <button
        type="button"
        class={swatchClass(editor.backgroundId === preset.id)}
        style:background={preset.swatch}
        title={preset.label}
        aria-label={preset.label}
        aria-pressed={editor.backgroundId === preset.id}
        onclick={() => pick(preset)}
      ></button>
    {/each}
  </div>
</PanelSection>

<!-- Bundled image wallpapers (radiant/mesh/pattern/paper). Each collapsed so its
     thumbnails only fetch when opened; images lazy-load and decode async. -->
{#each IMAGE_BACKGROUND_CATEGORIES as cat (cat.id)}
  <PanelSection title={cat.label} collapsible defaultOpen={false}>
    <div class="grid grid-cols-3 gap-2">
      {#each cat.images as img (img.id)}
        <button
          type="button"
          class={cn(
            "border-border aspect-video overflow-hidden border transition-transform hover:scale-105",
            editor.backgroundId === img.id ? "ring-primary rounded-lg ring-2 ring-offset-1" : "rounded-lg",
          )}
          aria-label={img.id}
          aria-pressed={editor.backgroundId === img.id}
          onclick={() => pickImage(img)}
        >
          <img
            src={img.url}
            alt=""
            loading="lazy"
            decoding="async"
            class="size-full object-cover"
          />
        </button>
      {/each}
    </div>
  </PanelSection>
{/each}

<!-- Patterns. -->
<PanelSection title="CSS Patterns" collapsible defaultOpen>
  <div class="grid grid-cols-6 gap-2">
    {#each PATTERN_PRESETS as preset (preset.id)}
      <button
        type="button"
        class={swatchClass(editor.backgroundId === preset.id)}
        style:background={preset.swatch}
        title={preset.label}
        aria-label={preset.label}
        aria-pressed={editor.backgroundId === preset.id}
        onclick={() => pick(preset)}
      ></button>
    {/each}
  </div>
</PanelSection>

<!-- Solid colors. -->
<PanelSection title="Solid" collapsible defaultOpen>
  <div class="grid grid-cols-6 gap-2">
    {#each SOLID_PRESETS as preset (preset.id)}
      <button
        type="button"
        class={swatchClass(editor.backgroundId === preset.id)}
        style:background={preset.swatch}
        title={preset.label}
        aria-label={preset.label}
        aria-pressed={editor.backgroundId === preset.id}
        onclick={() => pick(preset)}
      ></button>
    {/each}
  </div>
</PanelSection>
