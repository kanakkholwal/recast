<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface PerspectiveControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { Button } from "@recast/ui/button";
  import { RotateCcw } from "@recast/icons";
  import { DEFAULT_TRANSFORM, PERSPECTIVE_PRESETS } from "../presets";

  let { editor }: PerspectiveControlProps = $props();

  // Match a preset by comparing the numeric transform (so the active preset
  // highlights, and manual slider edits deselect all of them).
  function isActive(id: string): boolean {
    const p = PERSPECTIVE_PRESETS.find((x) => x.id === id);
    if (!p) return false;
    const t = editor.transform;
    return (
      p.transform.perspective === t.perspective &&
      p.transform.rotateX === t.rotateX &&
      p.transform.rotateY === t.rotateY &&
      p.transform.rotateZ === t.rotateZ &&
      p.transform.scale === t.scale &&
      p.transform.translateX === t.translateX &&
      p.transform.translateY === t.translateY
    );
  }
</script>

<PanelSection title="3D perspective">
  <div class="grid grid-cols-3 gap-1.5">
    {#each PERSPECTIVE_PRESETS as preset (preset.id)}
      <button
        type="button"
        class="ring-offset-background focus-visible:ring-ring rounded-lg border px-2 py-1.5 text-xs font-medium transition focus-visible:ring-2 focus-visible:outline-none"
        class:bg-primary={isActive(preset.id)}
        class:text-primary-foreground={isActive(preset.id)}
        class:border-transparent={isActive(preset.id)}
        class:bg-card={!isActive(preset.id)}
        class:text-foreground={!isActive(preset.id)}
        class:border-border={!isActive(preset.id)}
        class:hover:bg-muted={!isActive(preset.id)}
        aria-pressed={isActive(preset.id)}
        onclick={() => editor.setTransform(preset.transform)}
      >
        {preset.label}
      </button>
    {/each}
  </div>

  <SliderControl
    label="Tilt X"
    value={editor.transform.rotateX}
    min={-45}
    max={45}
    step={1}
    unit="°"
    onchange={(v) => editor.patchTransform({ rotateX: v })}
  />
  <SliderControl
    label="Tilt Y"
    value={editor.transform.rotateY}
    min={-45}
    max={45}
    step={1}
    unit="°"
    onchange={(v) => editor.patchTransform({ rotateY: v })}
  />
  <SliderControl
    label="Rotate"
    value={editor.transform.rotateZ}
    min={-45}
    max={45}
    step={1}
    unit="°"
    onchange={(v) => editor.patchTransform({ rotateZ: v })}
  />
  <SliderControl
    label="Depth"
    value={editor.transform.perspective}
    min={50}
    max={1000}
    step={10}
    unit="px"
    onchange={(v) => editor.patchTransform({ perspective: v })}
  />
  <SliderControl
    label="Scale"
    value={editor.transform.scale}
    min={0.5}
    max={1.5}
    step={0.01}
    onchange={(v) => editor.patchTransform({ scale: v })}
  />
  <SliderControl
    label="Translate X"
    value={editor.transform.translateX}
    min={-10}
    max={10}
    step={0.5}
    unit="%"
    onchange={(v) => editor.patchTransform({ translateX: v })}
  />
  <SliderControl
    label="Translate Y"
    value={editor.transform.translateY}
    min={-10}
    max={10}
    step={0.5}
    unit="%"
    onchange={(v) => editor.patchTransform({ translateY: v })}
  />
  <Button
    variant="ghost"
    size="sm"
    class="w-full justify-center gap-1.5"
    onclick={() => editor.setTransform({ ...DEFAULT_TRANSFORM })}
  >
    <RotateCcw class="size-3.5" />
    Reset 3D
  </Button>
</PanelSection>
