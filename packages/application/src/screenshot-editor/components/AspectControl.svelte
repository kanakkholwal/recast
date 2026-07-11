<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface AspectControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { ASPECT_PRESETS } from "../presets";

  let { editor }: AspectControlProps = $props();
</script>

<PanelSection title="Aspect ratio">
  <div class="grid grid-cols-3 gap-1.5">
    {#each ASPECT_PRESETS as preset (preset.id)}
      <button
        type="button"
        class="ring-offset-background focus-visible:ring-ring rounded-lg border px-2 py-1.5 text-xs font-medium transition focus-visible:ring-2 focus-visible:outline-none"
        class:bg-primary={editor.aspect.id === preset.id}
        class:text-primary-foreground={editor.aspect.id === preset.id}
        class:border-transparent={editor.aspect.id === preset.id}
        class:bg-card={editor.aspect.id !== preset.id}
        class:text-foreground={editor.aspect.id !== preset.id}
        class:border-border={editor.aspect.id !== preset.id}
        class:hover:bg-muted={editor.aspect.id !== preset.id}
        aria-pressed={editor.aspect.id === preset.id}
        onclick={() => editor.setAspect(preset)}
      >
        {preset.label}
      </button>
    {/each}
  </div>
</PanelSection>
