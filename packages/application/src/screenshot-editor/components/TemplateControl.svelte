<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface TemplateControlProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { Button } from "@recast/ui/button";
  import { Star, Plus, Trash2 } from "@recast/icons";
  import { TEMPLATE_PRESETS } from "../presets";
  import { deleteCustomPreset, listCustomPresets, saveCustomPreset } from "../persistence";
  import type { CustomPreset } from "../types";

  let { editor }: TemplateControlProps = $props();

  let custom = $state<CustomPreset[]>(listCustomPresets());
  let newName = $state("");

  function save() {
    saveCustomPreset(newName || `Preset ${custom.length + 1}`, editor.designObject(), Date.now());
    custom = listCustomPresets();
    newName = "";
  }

  function remove(id: string) {
    deleteCustomPreset(id);
    custom = listCustomPresets();
  }
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
        <span class="text-muted-foreground group-hover:text-foreground bg-card px-1 py-1 text-xs font-medium">
          {t.label}
        </span>
      </button>
    {/each}
  </div>
</PanelSection>

<PanelSection title="My Presets" collapsible defaultOpen={custom.length > 0}>
  <div class="flex gap-1.5">
    <input
      class="border-border bg-card focus-visible:ring-ring h-8 min-w-0 flex-1 rounded-lg border px-2 text-xs focus-visible:ring-2 focus-visible:outline-none"
      placeholder="Preset name"
      bind:value={newName}
      onkeydown={(e) => e.key === "Enter" && save()}
    />
    <Button variant="outline" size="sm" onclick={save}>
      <Plus />
      Save
    </Button>
  </div>

  {#if custom.length === 0}
    <p class="text-muted-foreground text-xs">Save the current look to reuse it on any screenshot.</p>
  {:else}
    <ul class="flex flex-col gap-1">
      {#each custom as p (p.id)}
        <li class="border-border hover:bg-muted/50 flex items-center gap-1.5 rounded-md border px-1.5 py-1">
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2 text-left outline-none"
            onclick={() => editor.applyDesignObject(p.design)}
          >
            <Star class="text-muted-foreground size-3.5 shrink-0" />
            <span class="text-foreground truncate text-xs">{p.name}</span>
          </button>
          <Button
            variant="ghost"
            size="icon"
            class="text-muted-foreground hover:text-destructive size-6"
            aria-label={`Delete ${p.name}`}
            onclick={() => remove(p.id)}
          >
            <Trash2 class="size-3.5" />
          </Button>
        </li>
      {/each}
    </ul>
  {/if}
</PanelSection>
