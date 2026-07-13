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
  import { ArrowUpRight, Circle, Square, Trash2 } from "@lucide/svelte";
  import type { ShapeKind, ShapeOverlay } from "../types";

  let { editor }: ShapeControlProps = $props();

  const selected = $derived(
    editor.selectedOverlay?.type === "shape" ? (editor.selectedOverlay as ShapeOverlay) : null,
  );

  const TOOLS: { shape: ShapeKind; label: string; icon: typeof Square }[] = [
    { shape: "rectangle", label: "Box", icon: Square },
    { shape: "ellipse", label: "Circle", icon: Circle },
    { shape: "arrow", label: "Arrow", icon: ArrowUpRight },
  ];

  function update(patch: Partial<ShapeOverlay>) {
    if (selected) editor.updateOverlay(selected.id, patch);
  }
</script>

<PanelSection title="Annotate">
  <div class="grid grid-cols-3 gap-1.5">
    {#each TOOLS as tool (tool.shape)}
      {@const Icon = tool.icon}
      <Button variant="outline" size="sm" onclick={() => editor.addShape(tool.shape)}>
        <Icon />
        {tool.label}
      </Button>
    {/each}
  </div>

  {#if selected}
    {@const sel = selected}
    <ColorField label="Color" value={sel.color} oncommit={(c) => update({ color: c })} />
    <SliderControl
      label="Stroke"
      value={sel.strokeWidth}
      min={1}
      max={16}
      step={1}
      unit="px"
      onchange={(v) => update({ strokeWidth: v })}
    />
    {#if sel.shape !== "arrow"}
      <div class="flex items-center justify-between">
        <span class="text-muted-foreground text-xs">Fill</span>
        <SegmentedToggle
          checked={sel.filled}
          onCheckedChange={(v) => update({ filled: v })}
          aria-label="Fill shape"
        />
      </div>
    {/if}
    <Button variant="ghost" size="sm" class="w-full" onclick={() => editor.removeOverlay(sel.id)}>
      <Trash2 />
      Delete shape
    </Button>
  {/if}
</PanelSection>
