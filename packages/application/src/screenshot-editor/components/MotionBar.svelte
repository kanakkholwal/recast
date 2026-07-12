<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface MotionBarProps {
    editor: ScreenshotEditorState;
    onclose: () => void;
  }
</script>

<script lang="ts">
  import { Button } from "@recast/ui/button";
  import { Pause, Play, X } from "@lucide/svelte";
  import { presetsByCategory } from "../animation";

  let { editor, onclose }: MotionBarProps = $props();

  const groups = presetsByCategory();
  let activeCategory = $state(groups[0].category);
  const activeGroup = $derived(groups.find((g) => g.category === activeCategory) ?? groups[0]);

  const seconds = (ms: number) => (ms / 1000).toFixed(1);
</script>

<div class="border-border bg-card flex flex-col gap-3 border-t p-3">
  <!-- Transport -->
  <div class="flex items-center gap-3">
    <Button
      variant="default"
      size="icon"
      aria-label={editor.playing ? "Pause" : "Play"}
      disabled={!editor.animationId}
      onclick={() => editor.togglePlay()}
    >
      {#if editor.playing}
        <Pause />
      {:else}
        <Play />
      {/if}
    </Button>

    <input
      type="range"
      class="accent-primary h-1.5 flex-1 cursor-pointer"
      min="0"
      max={editor.animationDuration || 100}
      step="10"
      value={editor.playhead}
      disabled={!editor.animationId}
      aria-label="Playhead"
      oninput={(e) => editor.seek(Number(e.currentTarget.value))}
    />

    <span class="text-muted-foreground w-16 text-right font-mono text-xs tabular-nums">
      {seconds(editor.playhead)}s / {seconds(editor.animationDuration)}s
    </span>

    {#if editor.animationId}
      <Button variant="ghost" size="sm" onclick={() => editor.clearAnimation()}>Clear</Button>
    {/if}
    <Button variant="ghost" size="icon" aria-label="Close motion" onclick={onclose}>
      <X />
    </Button>
  </div>

  <!-- Category tabs -->
  <div class="flex gap-1 overflow-x-auto pb-0.5">
    {#each groups as g (g.category)}
      <button
        type="button"
        class="shrink-0 rounded-md px-2.5 py-1 text-xs font-medium transition"
        class:bg-primary={activeCategory === g.category}
        class:text-primary-foreground={activeCategory === g.category}
        class:text-muted-foreground={activeCategory !== g.category}
        class:hover:bg-muted={activeCategory !== g.category}
        onclick={() => (activeCategory = g.category)}
      >
        {g.label}
      </button>
    {/each}
  </div>

  <!-- Presets in the active category -->
  <div class="grid grid-cols-4 gap-1.5 sm:grid-cols-6">
    {#each activeGroup.presets as p (p.id)}
      <button
        type="button"
        class="rounded-lg border px-2 py-2 text-[11px] font-medium transition"
        class:border-primary={editor.animationId === p.id}
        class:bg-primary={editor.animationId === p.id}
        class:text-primary-foreground={editor.animationId === p.id}
        class:border-border={editor.animationId !== p.id}
        class:bg-background={editor.animationId !== p.id}
        class:hover:bg-muted={editor.animationId !== p.id}
        aria-pressed={editor.animationId === p.id}
        onclick={() => editor.setAnimation(p.id)}
      >
        {p.name}
      </button>
    {/each}
  </div>
</div>
