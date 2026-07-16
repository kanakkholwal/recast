<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface BackgroundControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { ColorPicker } from "@recast/ui/color-picker";
  import { cn } from "@recast/ui/utils";
  import { ImageUp, X } from "@recast/icons";
  import { GRADIENT_PRESETS, MESH_PRESETS, PATTERN_PRESETS, SOLID_PRESETS } from "../presets";
  import { imageFromFile } from "../image-input";
  import type { BackgroundPreset } from "../types";

  let { editor }: BackgroundControlProps = $props();

  let bgFileInput = $state<HTMLInputElement | null>(null);
  let lastColor = $state("#7dd4ad");

  const gradients = [...GRADIENT_PRESETS, ...MESH_PRESETS];

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
        ? "border-primary/50 bg-primary/5 ring-primary/20 ring-1"
        : "border-border/40 bg-muted/30 hover:bg-accent hover:border-border/60",
    );

  const swatchClass = (active: boolean) =>
    cn(
      "border-border/30 aspect-square border transition-transform hover:scale-105",
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
          "text-[10px] font-medium",
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
      <span class="border-border/50 size-7 rounded-lg border" style:background={currentColor}></span>
      <span
        class={cn(
          "text-[10px] font-medium",
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
          class="border-border/50 size-3.5 rounded-full border"
          style="background:repeating-conic-gradient(#808080 0% 25%, #fff 0% 50%) 50% / 6px 6px;"
        ></span>
      </span>
      <span
        class={cn(
          "text-[10px] font-medium",
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
    <div class="border-border/40 bg-muted/50 relative aspect-video overflow-hidden rounded-lg">
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

<!-- Patterns. -->
<PanelSection title="Patterns" collapsible defaultOpen>
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
