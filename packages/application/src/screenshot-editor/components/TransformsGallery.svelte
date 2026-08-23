<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface TransformsGalleryProps {
	editor: ScreenshotEditorState;
}
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { cn } from "@recast/ui/utils";
  import { TRANSFORM_PRESET_CATEGORIES } from "../transform-presets";
  import { transformCss } from "../render";
  import type { Transform3D } from "../types";

  let { editor }: TransformsGalleryProps = $props();

  // Backdrop CSS for the mini-preview tiles (mirror the current stage backdrop).
  const previewBg = $derived.by(() => {
    const b = editor.background;
    if (b.kind === "solid") return b.color;
    if (b.kind === "gradient") return b.css;
    return "repeating-conic-gradient(#e5e7eb 0% 25%, #fff 0% 50%) 50% / 10px 10px";
  });

  // Active when the rotations match a preset (matches the reference tolerance).
  function isActive(t: Transform3D): boolean {
    const c = editor.transform;
    return (
      Math.abs(t.rotateX - c.rotateX) < 2 &&
      Math.abs(t.rotateY - c.rotateY) < 2 &&
      Math.abs(t.rotateZ - c.rotateZ) < 2 &&
      Math.abs(t.scale - c.scale) < 0.03
    );
  }
</script>

{#each TRANSFORM_PRESET_CATEGORIES as cat (cat.name)}
  <PanelSection title={cat.name} collapsible defaultOpen={cat.name === "Popular"}>
    <div class="grid grid-cols-3 gap-2">
      {#each cat.presets as preset (preset.id)}
        <button
          type="button"
          class={cn(
            "group/tile relative aspect-[4/3] overflow-hidden rounded-md border transition",
            isActive(preset.transform)
              ? "border-primary ring-foreground/20 ring-1"
              : "border-border hover:border-border",
          )}
          title={preset.name}
          aria-label={preset.name}
          aria-pressed={isActive(preset.transform)}
          onclick={() => editor.setTransform({ ...preset.transform })}
        >
          <span class="absolute inset-0" style:background={previewBg}></span>
          <span
            class="absolute inset-0 flex items-center justify-center"
            style:perspective={`${preset.transform.perspective}px`}
          >
            {#if editor.image}
              <img
                src={editor.image.src}
                alt=""
                class="h-[80%] w-[80%] rounded-sm object-contain shadow"
                style:transform={transformCss(preset.transform)}
              />
            {:else}
              <span
                class="border-border bg-foreground/10 h-[80%] w-[80%] rounded-sm border"
                style:transform={transformCss(preset.transform)}
              ></span>
            {/if}
          </span>
          <span
            class="bg-background/85 text-muted-foreground group-hover/tile:text-foreground absolute inset-x-0 bottom-0 truncate px-1 py-0.5 text-center text-xs font-medium opacity-0 transition-opacity group-hover/tile:opacity-100"
            class:opacity-100={isActive(preset.transform)}
          >
            {preset.name}
          </span>
        </button>
      {/each}
    </div>
  </PanelSection>
{/each}
