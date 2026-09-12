<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface AnimationControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { PropRow } from "@recast/ui/prop-row";
  import { PropSelect } from "@recast/ui/prop-select";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { Pause, Play, X } from "@recast/icons";
  import { presetsByCategory } from "../animation";

  let { editor }: AnimationControlProps = $props();

  const groups = presetsByCategory();
  let activeCategory = $state(groups[0].category);
  const activeGroup = $derived(groups.find((g) => g.category === activeCategory) ?? groups[0]);
  const categoryOptions = groups.map((g) => ({ value: g.category, label: g.label }));

  const seconds = (ms: number) => (ms / 1000).toFixed(1);
</script>

<!-- Status + transport for the selected motion. -->
<PanelSection variant="panel" title="Motion" flush>
  <div
    class="border-border bg-muted flex items-center gap-2 rounded-lg border px-2.5 py-2"
  >
    {#if editor.animationPreset}
      <Button
        variant="default"
        size="icon"
        class="size-7"
        aria-label={editor.playing ? "Pause" : "Play"}
        onclick={() => editor.togglePlay()}
      >
        {#if editor.playing}<Pause class="size-3.5" />{:else}<Play class="size-3.5" />{/if}
      </Button>
      <div class="min-w-0 flex-1">
        <p class="text-foreground truncate text-xs font-medium">{editor.animationPreset.name}</p>
        <p class="text-muted-foreground text-xs tabular-nums">
          {seconds(editor.playhead)}s / {seconds(editor.timelineDuration)}s
        </p>
      </div>
      <Button
        variant="ghost"
        size="icon"
        class="size-7"
        aria-label="Clear animation"
        onclick={() => editor.clearAnimation()}
      >
        <X class="size-3.5" />
      </Button>
    {:else}
      <p class="text-muted-foreground px-0.5 py-1 text-xs">
        Pick a motion below to animate the shot.
      </p>
    {/if}
  </div>
</PanelSection>

<!-- Category is a filter (which set of presets to show), so it's a select, not
     chips that would rhyme with the preset-card selection below. -->
<PanelSection variant="panel" title="Presets">
  <PropRow label="Category">
    <PropSelect
      class="flex-1"
      label="Motion category"
      value={activeCategory}
      options={categoryOptions}
      onChange={(v) => (activeCategory = v as typeof activeCategory)}
    />
  </PropRow>

  <div class="grid grid-cols-2 gap-1.5">
    {#each activeGroup.presets as p (p.id)}
      {@const selected = editor.animationId === p.id}
      <button
        type="button"
        class={cn(
          "rounded-lg border px-2 py-2 text-left text-xs font-medium transition-[colors,transform] motion-safe:active:scale-[0.98]",
          selected
            ? "border-transparent bg-foreground text-background"
            : "border-border bg-background hover:bg-accent text-foreground",
        )}
        aria-pressed={selected}
        onclick={() => editor.setAnimation(p.id)}
      >
        <span class="block truncate">{p.name}</span>
        <span
          class={cn(
            "text-xs tabular-nums",
            selected ? "text-background/75" : "text-muted-foreground",
          )}
        >
          {seconds(p.duration)}s
        </span>
      </button>
    {/each}
  </div>
</PanelSection>
