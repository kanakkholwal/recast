<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface FrameControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";

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
    label="Shadow"
    value={editor.frame.shadow}
    min={0}
    max={100}
    step={1}
    onchange={(v) => editor.patchFrame({ shadow: v })}
  />
</PanelSection>
