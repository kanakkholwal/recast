<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface TemplateControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { TEMPLATE_PRESETS } from "../presets";

  let { editor }: TemplateControlProps = $props();
</script>

<PanelSection title="Templates">
  <div class="grid grid-cols-3 gap-2">
    {#each TEMPLATE_PRESETS as t (t.id)}
      <button
        type="button"
        class="ring-offset-background focus-visible:ring-ring group flex flex-col overflow-hidden rounded-lg border transition focus-visible:ring-2 focus-visible:outline-none"
        class:border-primary={editor.backgroundId === t.backgroundId}
        class:border-border={editor.backgroundId !== t.backgroundId}
        onclick={() => editor.applyTemplate(t)}
      >
        <span class="block h-10 w-full" style:background={t.swatch}></span>
        <span class="text-muted-foreground group-hover:text-foreground bg-card px-1 py-1 text-[11px] font-medium">
          {t.label}
        </span>
      </button>
    {/each}
  </div>
</PanelSection>
