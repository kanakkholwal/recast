<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";
import type { ShadowPreset } from "../types";

export interface ShadowPresetControlProps {
	editor: ScreenshotEditorState;
}

// Preview-only box-shadow strings (match the clone's ShadowPreview tiles).
const PRESETS: { value: ShadowPreset; label: string; shadow: string }[] = [
	{ value: "none", label: "None", shadow: "none" },
	{
		value: "hug",
		label: "Hug",
		shadow: "rgba(0,0,0,0.2) 0px 2px 12px 0px, rgba(0,0,0,0.14) 0px 1px 4px 0px",
	},
	{
		value: "soft",
		label: "Soft",
		shadow: "rgba(0,0,0,0.28) 0px 12px 48px 0px, rgba(0,0,0,0.18) 0px 4px 12px 0px",
	},
	{
		value: "strong",
		label: "Strong",
		shadow: "rgba(0,0,0,0.45) 0px 24px 80px 0px, rgba(0,0,0,0.3) 0px 8px 24px 0px",
	},
];
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { cn } from "@recast/ui/utils";

  let { editor }: ShadowPresetControlProps = $props();
</script>

<PanelSection title="Shadow" collapsible defaultOpen>
  <div class="grid grid-cols-2 gap-2">
    {#each PRESETS as preset (preset.value)}
      {@const selected = editor.shadowPreset === preset.value}
      <button
        type="button"
        class="group/shadow flex flex-col items-center gap-1.5 outline-none"
        onclick={() => editor.setShadowPreset(preset.value)}
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
          <span
            class="absolute rounded-[10px] bg-white"
            style={`top:26%;left:26%;width:95%;height:95%;box-shadow:${preset.shadow};`}
          ></span>
        </span>
        <span
          class={cn(
            "text-xs leading-tight transition-colors",
            selected
              ? "text-foreground font-medium"
              : "text-muted-foreground group-hover/shadow:text-foreground",
          )}
        >
          {preset.label}
        </span>
      </button>
    {/each}
  </div>
</PanelSection>
