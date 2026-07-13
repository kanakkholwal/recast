<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface BorderControlProps {
    editor: ScreenshotEditorState;
  }

  const RADIUS_PRESETS = [
    { value: 0, label: "Sharp", preview: "0px" },
    { value: 12, label: "Curved", preview: "6px" },
    { value: 20, label: "Round", preview: "12px" },
  ] as const;
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";

  let { editor }: BorderControlProps = $props();
</script>

<PanelSection title="Border" collapsible defaultOpen>
  <div class="grid grid-cols-3 gap-2">
    {#each RADIUS_PRESETS as preset (preset.value)}
      {@const selected = editor.frame.radius === preset.value}
      <button
        type="button"
        class="group/border flex flex-col items-center gap-1.5 outline-none"
        onclick={() => editor.patchFrame({ radius: preset.value })}
      >
        <span
          class={cn(
            "relative block aspect-square w-full overflow-hidden rounded-lg transition-all",
            selected
              ? "ring-primary ring-offset-card ring-[1.5px] ring-offset-1"
              : "ring-border/50 ring-1",
          )}
          style="background:rgb(210,210,214);"
        >
          <span class="absolute" style="top:19.5%;left:19.5%;width:95.5%;height:95.5%;">
            <span class="block size-full bg-white" style={`border-radius:${preset.preview};`}></span>
          </span>
        </span>
        <span
          class={cn(
            "text-[10px] leading-tight transition-colors",
            selected
              ? "text-foreground font-medium"
              : "text-muted-foreground group-hover/border:text-foreground/70",
          )}
        >
          {preset.label}
        </span>
      </button>
    {/each}
  </div>

  <SliderControl
    label="Radius"
    value={editor.frame.radius}
    min={0}
    max={50}
    step={1}
    unit="px"
    onchange={(v) => editor.patchFrame({ radius: v })}
  />
  <SliderControl
    label="Scale"
    value={editor.imageScale / 100}
    min={0.1}
    max={2}
    step={0.01}
    formatValue={(v) => v.toFixed(2)}
    onchange={(v) => editor.setImageScale(Math.round(v * 100))}
  />
</PanelSection>
