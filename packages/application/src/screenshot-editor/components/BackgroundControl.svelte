<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface BackgroundControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { ColorPicker } from "@recast/ui/color-picker";
  import { Segmented } from "@recast/ui/segmented";
  import { GRADIENT_PRESETS, SOLID_PRESETS } from "../presets";
  import type { BackgroundPreset } from "../types";

  let { editor }: BackgroundControlProps = $props();

  type Mode = "gradient" | "solid" | "transparent";

  const mode = $derived<Mode>(
    editor.background.kind === "transparent"
      ? "transparent"
      : editor.backgroundId === "custom" || SOLID_PRESETS.some((p) => p.id === editor.backgroundId)
        ? "solid"
        : "gradient",
  );

  function setMode(next: Mode) {
    if (next === "transparent") {
      editor.setBackground("transparent", { kind: "transparent" });
    } else if (next === "solid") {
      const p = SOLID_PRESETS[0];
      editor.setBackground(p.id, p.background);
    } else {
      const p = GRADIENT_PRESETS[0];
      editor.setBackground(p.id, p.background);
    }
  }

  function pick(preset: BackgroundPreset) {
    editor.setBackground(preset.id, preset.background);
  }

  const currentColor = $derived(
    editor.background.kind === "solid" ? editor.background.color : "#f4f4f5",
  );
</script>

<PanelSection title="Background">
  <Segmented
    options={[
      { value: "gradient", label: "Gradient" },
      { value: "solid", label: "Solid" },
      { value: "transparent", label: "None" },
    ]}
    value={mode}
    onValueChange={(v) => setMode(v as Mode)}
    aria-label="Background type"
  />

  {#if mode === "gradient"}
    <div class="grid grid-cols-8 gap-1.5">
      {#each GRADIENT_PRESETS as preset (preset.id)}
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
  {:else if mode === "solid"}
    <div class="grid grid-cols-8 gap-1.5">
      {#each SOLID_PRESETS as preset (preset.id)}
        <button
          type="button"
          class="ring-offset-background focus-visible:ring-ring aspect-square rounded-md border transition focus-visible:ring-2 focus-visible:outline-none"
          class:ring-2={editor.backgroundId === preset.id}
          class:ring-primary={editor.backgroundId === preset.id}
          style:background={preset.swatch}
          title={preset.label}
          aria-label={preset.label}
          aria-pressed={editor.backgroundId === preset.id}
          onclick={() => pick(preset)}
        ></button>
      {/each}
    </div>
    <ColorPicker
      value={currentColor}
      oncommit={(next) => editor.setCustomColor(next)}
      allowAlpha={false}
    />
  {/if}
</PanelSection>
