<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";
import type { AspectPreset } from "../types";

export interface AspectControlProps {
	editor: ScreenshotEditorState;
}

// The 25 presets read as three intents; a flat 25-button grid did not.
const GROUPS: { label: string; ids: string[] }[] = [
	{
		label: "Basic",
		ids: ["auto", "1_1", "4_5", "9_16", "16_9", "3_4", "2_3", "3_2", "4_3", "5_4", "16_10"],
	},
	{
		label: "Social",
		ids: [
			"og_image",
			"twitter_banner",
			"instagram_banner",
			"youtube_banner",
			"linkedin_banner",
			"youtube_thumbnail",
			"youtube_video",
			"pinterest_long",
		],
	},
	{
		label: "Device",
		ids: [
			"appstore_iphone65",
			"appstore_iphone55",
			"appstore_ipad",
			"appstore_iphone65_landscape",
			"appstore_iphone55_landscape",
			"appstore_ipad_landscape",
		],
	},
];

const FRAME = 16;
/** Proportional rectangle for the ratio, fit inside a 16px frame. */
function shape(ratio: number | null): { w: number; h: number } {
	if (ratio == null) return { w: FRAME, h: FRAME };
	if (ratio >= 1) return { w: FRAME, h: Math.max(4, FRAME / ratio) };
	return { w: Math.max(4, FRAME * ratio), h: FRAME };
}
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { Combobox } from "@recast/ui/combobox";
  import * as Command from "@recast/ui/command";
  import { cn } from "@recast/ui/utils";
  import { Check, Ratio } from "@recast/icons";
  import { ASPECT_PRESETS } from "../presets";

  let { editor }: AspectControlProps = $props();

  const byId = new Map(ASPECT_PRESETS.map((p: AspectPreset) => [p.id, p]));
  let open = $state(false);

  function pick(preset: AspectPreset) {
    editor.setAspect(preset);
    open = false;
  }
</script>

<Combobox bind:open align="end" placeholder="Search ratios…" emptyText="No ratios" contentClass="w-64">
  {#snippet trigger({ props })}
    <Button {...props} variant="ghost" size="sm">
      <Ratio />
      {editor.aspect.label}
    </Button>
  {/snippet}
  {#each GROUPS as group (group.label)}
    <Command.Group heading={group.label}>
      {#each group.ids as id (id)}
        {@const preset = byId.get(id)}
        {#if preset}
          {@const selected = editor.aspect.id === id}
          {@const s = shape(preset.ratio)}
          <Command.Item value={`${preset.label} ${id}`} onSelect={() => pick(preset)}>
            <span class="grid size-4 shrink-0 place-items-center">
              <span
                class={cn(
                  "rounded-[2px] border",
                  preset.ratio == null && "border-dashed",
                  selected ? "border-foreground bg-foreground/20" : "border-muted-foreground/50",
                )}
                style:width={`${s.w}px`}
                style:height={`${s.h}px`}
              ></span>
            </span>
            <span class="min-w-0 flex-1 truncate">{preset.label}</span>
            {#if selected}
              <Check class="text-foreground size-3.5 shrink-0" />
            {/if}
          </Command.Item>
        {/if}
      {/each}
    </Command.Group>
  {/each}
</Combobox>
