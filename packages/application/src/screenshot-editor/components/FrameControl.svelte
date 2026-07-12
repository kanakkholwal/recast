<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface FrameControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { ColorField } from "@recast/ui/color-field";

  let { editor }: FrameControlProps = $props();
</script>

<PanelSection title="Frame">
  <SliderControl
    label="Padding"
    value={editor.frame.padding}
    min={0}
    max={25}
    step={1}
    unit="%"
    onchange={(v) => editor.patchFrame({ padding: v })}
  />
  <SliderControl
    label="Rounding"
    value={editor.frame.radius}
    min={0}
    max={40}
    step={1}
    unit="px"
    onchange={(v) => editor.patchFrame({ radius: v })}
  />
  <SliderControl
    label="Border"
    value={editor.frame.border.width}
    min={0}
    max={20}
    step={1}
    unit="px"
    onchange={(v) => editor.patchBorder({ width: v })}
  />
  {#if editor.frame.border.width > 0}
    <ColorField
      label="Border color"
      value={editor.frame.border.color}
      oncommit={(c) => editor.patchBorder({ color: c })}
    />
  {/if}
</PanelSection>
