<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface ShadowControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { ColorField } from "@recast/ui/color-field";

  let { editor }: ShadowControlProps = $props();
</script>

<PanelSection title="Shadow" collapsible defaultOpen={false}>
  <SliderControl
    label="Opacity"
    value={editor.shadow.opacity}
    min={0}
    max={1}
    step={0.01}
    onchange={(v) => editor.patchShadow({ opacity: v })}
  />
  <SliderControl
    label="Blur"
    value={editor.shadow.blur}
    min={0}
    max={120}
    step={1}
    unit="px"
    onchange={(v) => editor.patchShadow({ blur: v })}
  />
  <SliderControl
    label="Distance"
    value={editor.shadow.y}
    min={0}
    max={120}
    step={1}
    unit="px"
    onchange={(v) => editor.patchShadow({ y: v })}
  />
  <SliderControl
    label="Spread"
    value={editor.shadow.spread}
    min={-40}
    max={40}
    step={1}
    unit="px"
    onchange={(v) => editor.patchShadow({ spread: v })}
  />
  {#if editor.shadow.opacity > 0}
    <ColorField
      label="Shadow color"
      value={editor.shadow.color}
      oncommit={(c) => editor.patchShadow({ color: c })}
    />
  {/if}
</PanelSection>
