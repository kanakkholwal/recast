<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface BackgroundControlProps {
    editor: ScreenshotEditorState;
  }

  type Mode = "gradient" | "pattern" | "solid" | "image" | "transparent";
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { ColorPicker } from "@recast/ui/color-picker";
  import { Segmented } from "@recast/ui/segmented";
  import { Button } from "@recast/ui/button";
  import { ImageUp } from "@lucide/svelte";
  import { GRADIENT_PRESETS, MESH_PRESETS, PATTERN_PRESETS, SOLID_PRESETS } from "../presets";
  import { imageFromFile } from "../image-input";
  import type { BackgroundPreset } from "../types";

  let { editor }: BackgroundControlProps = $props();

  let bgFileInput = $state<HTMLInputElement | null>(null);

  const gradients = [...GRADIENT_PRESETS, ...MESH_PRESETS];

  const mode = $derived<Mode>(
    editor.background.kind === "transparent"
      ? "transparent"
      : editor.backgroundId === "image"
        ? "image"
        : PATTERN_PRESETS.some((p) => p.id === editor.backgroundId)
          ? "pattern"
          : editor.backgroundId === "custom" ||
              SOLID_PRESETS.some((p) => p.id === editor.backgroundId)
            ? "solid"
            : "gradient",
  );

  function setMode(next: Mode) {
    if (next === mode) return;
    if (next === "transparent") editor.setBackground("transparent", { kind: "transparent" });
    else if (next === "solid") pick(SOLID_PRESETS[0]);
    else if (next === "pattern") pick(PATTERN_PRESETS[0]);
    else if (next === "gradient") pick(gradients[0]);
    else if (next === "image") bgFileInput?.click();
  }

  function pick(preset: BackgroundPreset) {
    editor.setBackground(preset.id, preset.background);
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

  const currentColor = $derived(
    editor.background.kind === "solid" ? editor.background.color : "#f4f4f5",
  );

  const swatches = $derived(
    mode === "pattern" ? PATTERN_PRESETS : mode === "gradient" ? gradients : SOLID_PRESETS,
  );
</script>

<input bind:this={bgFileInput} type="file" accept="image/*" class="hidden" onchange={onBgFile} />

<PanelSection title="Background">
  <Segmented
    options={[
      { value: "gradient", label: "Gradient" },
      { value: "pattern", label: "Pattern" },
      { value: "solid", label: "Solid" },
      { value: "image", label: "Image" },
      { value: "transparent", label: "None" },
    ]}
    value={mode}
    onValueChange={(v) => setMode(v as Mode)}
    size="xs"
    aria-label="Background type"
  />

  {#if mode === "gradient" || mode === "pattern" || mode === "solid"}
    <div class="grid grid-cols-8 gap-1.5">
      {#each swatches as preset (preset.id)}
        <button
          type="button"
          class="ring-offset-background focus-visible:ring-ring aspect-square rounded-md border transition focus-visible:ring-2 focus-visible:outline-none"
          class:ring-2={editor.backgroundId === preset.id}
          class:ring-primary={editor.backgroundId === preset.id}
          class:border-transparent={editor.backgroundId === preset.id}
          class:border-border={editor.backgroundId !== preset.id}
          style:background={preset.swatch}
          title={preset.label}
          aria-label={preset.label}
          aria-pressed={editor.backgroundId === preset.id}
          onclick={() => pick(preset)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if mode === "solid"}
    <ColorPicker
      value={currentColor}
      oncommit={(next) => editor.setCustomColor(next)}
      allowAlpha={false}
    />
  {/if}

  {#if mode === "image"}
    <Button variant="outline" size="sm" onclick={() => bgFileInput?.click()}>
      <ImageUp />
      {editor.backgroundId === "image" ? "Replace image" : "Choose image"}
    </Button>
  {/if}
</PanelSection>
