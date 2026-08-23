<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";
import type { ImageStylePreset } from "../types";

export interface StyleControlProps {
	editor: ScreenshotEditorState;
}

const PRESETS: { value: ImageStylePreset; label: string }[] = [
	{ value: "default", label: "Default" },
	{ value: "glass-light", label: "Glass Light" },
	{ value: "glass-dark", label: "Glass Dark" },
	{ value: "outline", label: "Outline" },
	{ value: "border-light", label: "Border" },
	{ value: "border-dark", label: "Border Dark" },
];

// Wrapper look for each mini preview tile (illustration chrome, fixed colors).
function wrapperStyle(preset: ImageStylePreset): string {
	switch (preset) {
		case "glass-light":
			return "background:rgba(255,255,255,0.3);padding:3px;border-radius:7px;";
		case "glass-dark":
			return "background:rgba(0,0,0,0.35);padding:3px;border-radius:7px;";
		case "outline":
			return "background:rgba(255,255,255,0.4);padding:2px;border-radius:7px;";
		case "border-light":
			return "background:rgb(255,255,255);padding:4px;border-radius:8px;";
		case "border-dark":
			return "background:rgb(30,30,30);padding:4px;border-radius:8px;";
		default:
			return "";
	}
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";

  let { editor }: StyleControlProps = $props();

  const nonDefault = $derived(editor.imageStyle.preset !== "default");
</script>

<PanelSection title="Style" collapsible defaultOpen>
  <div class="grid grid-cols-3 gap-2">
    {#each PRESETS as preset (preset.value)}
      {@const selected = editor.imageStyle.preset === preset.value}
      <button
        type="button"
        class="group/style flex flex-col items-center gap-1.5 outline-none"
        onclick={() => editor.setImageStylePreset(preset.value)}
      >
        <span
          class={cn(
            "relative block aspect-square w-full overflow-hidden rounded-lg transition-all",
            selected
              ? "ring-primary ring-offset-card ring-[1.5px] ring-offset-1"
              : "ring-border/50 ring-1",
          )}
          style={`background:${preset.value === "glass-dark" || preset.value === "border-dark" ? "rgb(160,160,165)" : "rgb(210,210,214)"};`}
        >
          <span class="absolute" style="top:19.5%;left:19.5%;width:95.5%;height:95.5%;">
            {#if preset.value === "default"}
              <span class="block size-full rounded-[8px] bg-white"></span>
            {:else}
              <span class="block size-full" style={wrapperStyle(preset.value)}>
                <span class="block size-full rounded-[5px] bg-white"></span>
              </span>
            {/if}
          </span>
        </span>
        <span
          class={cn(
            "text-xs leading-tight transition-colors",
            selected
              ? "text-foreground font-medium"
              : "text-muted-foreground group-hover/style:text-foreground",
          )}
        >
          {preset.label}
        </span>
      </button>
    {/each}
  </div>

  {#if nonDefault}
    <SliderControl
      label="Padding"
      value={editor.imageStyle.padding}
      min={0}
      max={8}
      step={0.5}
      onchange={(v) => editor.patchImageStyle({ padding: v })}
    />
    <SliderControl
      label="Opacity"
      value={Math.round(editor.imageStyle.opacity * 100)}
      min={5}
      max={100}
      step={1}
      unit="%"
      onchange={(v) => editor.patchImageStyle({ opacity: v / 100 })}
    />
  {/if}
</PanelSection>
