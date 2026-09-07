<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface CanvasControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderRow } from "@recast/ui/slider-row";

  let { editor }: CanvasControlProps = $props();
</script>

<PanelSection variant="panel" title="Canvas" collapsible defaultOpen={false}>
  <SliderRow label="Background blur" value={editor.backgroundBlur} min={0} max={50} step={1} unit="px" onchange={(v) => editor.setBackgroundBlur(v)} />
  <SliderRow label="Grain" value={editor.backgroundNoise} min={0} max={100} step={1} unit="%" onchange={(v) => editor.setBackgroundNoise(v)} />
  <SliderRow label="Corner radius" value={editor.canvasRadius} min={0} max={100} step={1} unit="px" onchange={(v) => editor.setCanvasRadius(v)} />
</PanelSection>
