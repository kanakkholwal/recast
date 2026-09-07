<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface CanvasControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";

  let { editor }: CanvasControlProps = $props();
</script>

<PanelSection title="Canvas" collapsible defaultOpen={false}>
  <SliderControl
    label="Background blur"
    value={editor.backgroundBlur}
    min={0}
    max={50}
    step={1}
    unit="px"
    onchange={(v) => editor.setBackgroundBlur(v)}
  />
  <SliderControl
    label="Grain"
    value={editor.backgroundNoise}
    min={0}
    max={100}
    step={1}
    unit="%"
    onchange={(v) => editor.setBackgroundNoise(v)}
  />
  <SliderControl
    label="Corner radius"
    value={editor.canvasRadius}
    min={0}
    max={100}
    step={1}
    unit="px"
    onchange={(v) => editor.setCanvasRadius(v)}
  />
</PanelSection>
