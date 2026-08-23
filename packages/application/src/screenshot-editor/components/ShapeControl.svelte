<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface ShapeControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { ColorField } from "@recast/ui/color-field";
  import { Button } from "@recast/ui/button";
  import { ArrowUpRight, Circle, Droplets, Minus, Square, Trash2 } from "@recast/icons";
  import type { BlurOverlay, ShapeKind, ShapeOverlay } from "../types";

  let { editor }: ShapeControlProps = $props();

  const selected = $derived(
    editor.selectedOverlay?.type === "shape" ? (editor.selectedOverlay as ShapeOverlay) : null,
  );
  const selBlur = $derived(
    editor.selectedOverlay?.type === "blur" ? (editor.selectedOverlay as BlurOverlay) : null,
  );

  const TOOLS: { shape: ShapeKind; label: string; icon: typeof Square }[] = [
    { shape: "rectangle", label: "Box", icon: Square },
    { shape: "ellipse", label: "Circle", icon: Circle },
    { shape: "arrow", label: "Arrow", icon: ArrowUpRight },
    { shape: "line", label: "Line", icon: Minus },
  ];

  // Fill only makes sense for closed shapes.
  const canFill = $derived(selected?.shape === "rectangle" || selected?.shape === "ellipse");

  function update(patch: Partial<ShapeOverlay>) {
    if (selected) editor.updateOverlay(selected.id, patch);
  }
</script>

<PanelSection title="Annotate">
  <div class="grid grid-cols-5 gap-1.5">
    {#each TOOLS as tool (tool.shape)}
      {@const Icon = tool.icon}
      <Button variant="outline" size="sm" onclick={() => editor.addShape(tool.shape)}>
        <Icon />
      </Button>
    {/each}
    <Button variant="outline" size="sm" aria-label="Blur region" onclick={() => editor.addBlur()}>
      <Droplets />
    </Button>
  </div>

  {#if selBlur}
    {@const b = selBlur}
    <SliderControl
      label="Blur amount"
      value={b.blurAmount}
      min={2}
      max={40}
      step={1}
      unit="px"
      onchange={(v) => editor.updateOverlay(b.id, { blurAmount: v })}
    />
    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(b.id)}>
      <Trash2 />
      Delete blur
    </Button>
  {/if}

  {#if selected}
    {@const sel = selected}
    <ColorField label="Stroke color" value={sel.strokeColor} oncommit={(c) => update({ strokeColor: c })} />
    <SliderControl
      label="Stroke"
      value={sel.strokeWidth}
      min={1}
      max={16}
      step={1}
      unit="px"
      onchange={(v) => update({ strokeWidth: v })}
    />
    {#if canFill}
      <div class="flex items-center justify-between">
        <span class="text-muted-foreground text-xs">Fill</span>
        <SegmentedToggle
          checked={sel.filled}
          onCheckedChange={(v) => update({ filled: v })}
          aria-label="Fill shape"
        />
      </div>
      {#if sel.filled}
        <ColorField label="Fill color" value={sel.fillColor} oncommit={(c) => update({ fillColor: c })} />
      {/if}
    {/if}
    <SliderControl
      label="Opacity"
      value={Math.round(sel.opacity * 100)}
      min={0}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => update({ opacity: v / 100 })}
    />
    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(sel.id)}>
      <Trash2 />
      Delete shape
    </Button>
  {/if}
</PanelSection>
