<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface ShadowControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { PropRow } from "@recast/ui/prop-row";
  import { SliderRow } from "@recast/ui/slider-row";
  import { NumberField } from "@recast/ui/number-field";
  import { ColorField } from "@recast/ui/color-field";
  import { MoveHorizontal, MoveVertical, SquareRoundCorner } from "@recast/icons";

  let { editor }: ShadowControlProps = $props();
</script>

<PanelSection variant="panel" title="Custom shadow" collapsible defaultOpen={false}>
  <SliderRow
    label="Opacity"
    value={Math.round(editor.shadow.opacity * 100)}
    min={0}
    max={100}
    step={1}
    unit="%"
    onchange={(v) => editor.patchShadow({ opacity: v / 100 })}
  />
  <SliderRow
    label="Blur"
    value={editor.shadow.blur}
    min={0}
    max={50}
    step={1}
    unit="px"
    onchange={(v) => editor.patchShadow({ blur: v })}
  />
  <PropRow label="Spread">
    <NumberField
      class="flex-1"
      label="Shadow spread"
      icon={SquareRoundCorner}
      value={editor.shadow.spread}
      min={-10}
      max={20}
      suffix="px"
      onInput={(v) => editor.patchShadow({ spread: v })}
      onCommit={(v) => editor.patchShadow({ spread: v })}
    />
  </PropRow>
  <PropRow label="Offset X">
    <NumberField
      class="flex-1"
      label="Shadow horizontal offset"
      icon={MoveHorizontal}
      value={editor.shadow.x}
      min={-20}
      max={20}
      suffix="px"
      onInput={(v) => editor.patchShadow({ x: v })}
      onCommit={(v) => editor.patchShadow({ x: v })}
    />
  </PropRow>
  <PropRow label="Offset Y">
    <NumberField
      class="flex-1"
      label="Shadow vertical offset"
      icon={MoveVertical}
      value={editor.shadow.y}
      min={-20}
      max={20}
      suffix="px"
      onInput={(v) => editor.patchShadow({ y: v })}
      onCommit={(v) => editor.patchShadow({ y: v })}
    />
  </PropRow>
  <PropRow label="Color">
    <ColorField
      dense
      hideLabel
      class="flex-1"
      label="Shadow color"
      value={editor.shadow.color}
      oncommit={(c) => editor.patchShadow({ color: c })}
    />
  </PropRow>
</PanelSection>
