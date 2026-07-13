<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface AnimationControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { Pause, Play, X } from "@lucide/svelte";
  import { presetsByCategory } from "../animation";

  let { editor }: AnimationControlProps = $props();

  const groups = presetsByCategory();
  let activeCategory = $state(groups[0].category);
  const activeGroup = $derived(groups.find((g) => g.category === activeCategory) ?? groups[0]);

  const seconds = (ms: number) => (ms / 1000).toFixed(1);
</script>

<!-- Status + transport for the selected motion. -->
<PanelSection title="Motion" flush>
  <div
    class="border-border/50 bg-muted/30 flex items-center gap-2 rounded-lg border px-2.5 py-2"
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
        <p class="text-muted-foreground font-mono text-[10px] tabular-nums">
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

<!-- Category chips. -->
<div class="-mx-0.5 flex gap-1 overflow-x-auto px-0.5 pb-0.5">
  {#each groups as g (g.category)}
    <button
      type="button"
      class={cn(
        "shrink-0 rounded-md px-2 py-1 text-[11px] font-medium transition-colors",
        activeCategory === g.category
          ? "bg-primary text-primary-foreground"
          : "text-muted-foreground hover:bg-muted",
      )}
      onclick={() => (activeCategory = g.category)}
    >
      {g.label}
    </button>
  {/each}
</div>

<!-- Preset cards for the active category. -->
<div class="grid grid-cols-2 gap-1.5">
  {#each activeGroup.presets as p (p.id)}
    {@const selected = editor.animationId === p.id}
    <button
      type="button"
      class={cn(
        "rounded-lg border px-2 py-2 text-left text-[11px] font-medium transition-colors",
        selected
          ? "border-primary bg-primary text-primary-foreground"
          : "border-border bg-background hover:bg-muted text-foreground",
      )}
      aria-pressed={selected}
      onclick={() => editor.setAnimation(p.id)}
    >
      <span class="block truncate">{p.name}</span>
      <span
        class={cn(
          "font-mono text-[10px] tabular-nums",
          selected ? "text-primary-foreground/75" : "text-muted-foreground",
        )}
      >
        {seconds(p.duration)}s
      </span>
    </button>
  {/each}
</div>
