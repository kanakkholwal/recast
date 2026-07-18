<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface FilterControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { Button } from "@recast/ui/button";
  import { RotateCcw } from "@recast/icons";

  let { editor }: FilterControlProps = $props();

  // True when any adjustment is off its neutral value, so Reset can hide.
  const dirty = $derived(
    editor.filters.brightness !== 100 ||
      editor.filters.contrast !== 100 ||
      editor.filters.saturate !== 100 ||
      editor.filters.grayscale !== 0 ||
      editor.filters.sepia !== 0 ||
      editor.filters.hueRotate !== 0 ||
      editor.filters.invert !== 0 ||
      editor.filters.blur !== 0,
  );
</script>

<PanelSection title="Color Filters" collapsible defaultOpen={false}>
  {#snippet action()}
    {#if dirty}
      <Button
        variant="ghost"
        size="icon"
        class="size-6"
        aria-label="Reset color filters"
        title="Reset color filters"
        onclick={() => editor.resetFilters()}
      >
        <RotateCcw class="size-3.5" />
      </Button>
    {/if}
  {/snippet}
  <SliderControl
    label="Brightness"
    value={editor.filters.brightness}
    min={0}
    max={200}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ brightness: v })}
  />
  <SliderControl
    label="Contrast"
    value={editor.filters.contrast}
    min={0}
    max={200}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ contrast: v })}
  />
  <SliderControl
    label="Saturation"
    value={editor.filters.saturate}
    min={0}
    max={200}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ saturate: v })}
  />
  <SliderControl
    label="Grayscale"
    value={editor.filters.grayscale}
    min={0}
    max={100}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ grayscale: v })}
  />
  <SliderControl
    label="Sepia"
    value={editor.filters.sepia}
    min={0}
    max={100}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ sepia: v })}
  />
  <SliderControl
    label="Hue"
    value={editor.filters.hueRotate}
    min={0}
    max={360}
    step={1}
    unit="°"
    onchange={(v) => editor.patchFilters({ hueRotate: v })}
  />
  <SliderControl
    label="Invert"
    value={editor.filters.invert}
    min={0}
    max={100}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFilters({ invert: v })}
  />
  <SliderControl
    label="Blur"
    value={editor.filters.blur}
    min={0}
    max={20}
    step={0.5}
    unit="px"
    onchange={(v) => editor.patchFilters({ blur: v })}
  />
</PanelSection>
