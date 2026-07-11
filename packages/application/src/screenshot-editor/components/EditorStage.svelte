<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface EditorStageProps {
    editor: ScreenshotEditorState;
    /** The exportable node — bound out so the toolbar can snapshot it. */
    stageEl?: HTMLElement | null;
  }

  /** Map the 0..100 shadow dial to a layered, natural-looking drop shadow. */
  function shadowCss(strength: number): string {
    if (strength <= 0) return "none";
    const t = strength / 100;
    const y1 = Math.round(2 + 10 * t);
    const b1 = Math.round(6 + 20 * t);
    const y2 = Math.round(8 + 40 * t);
    const b2 = Math.round(20 + 60 * t);
    return `0 ${y1}px ${b1}px rgba(0,0,0,${(0.12 + 0.12 * t).toFixed(3)}), 0 ${y2}px ${b2}px rgba(0,0,0,${(0.1 + 0.18 * t).toFixed(3)})`;
  }
</script>

<script lang="ts">
  import MockupFrame from "./MockupFrame.svelte";

  let { editor, stageEl = $bindable(null) }: EditorStageProps = $props();

  // Aspect ratio for the stage: an explicit preset, else the screenshot's own.
  const aspectRatio = $derived.by(() => {
    if (editor.aspect.ratio != null) return editor.aspect.ratio;
    const img = editor.image;
    return img && img.height > 0 ? img.width / img.height : 16 / 9;
  });

  const backgroundCss = $derived.by(() => {
    const bg = editor.background;
    if (bg.kind === "solid") return bg.color;
    if (bg.kind === "gradient") return bg.css;
    return "transparent";
  });
</script>

<!-- The stage IS the export node: what renders here is what gets snapshotted. -->
<div
  bind:this={stageEl}
  class="recast-shot-stage"
  class:recast-shot-checkerboard={editor.background.kind === "transparent"}
  style:aspect-ratio={`${aspectRatio}`}
  style:padding={`${editor.frame.padding}%`}
  style:background={editor.background.kind === "transparent" ? undefined : backgroundCss}
>
  {#if editor.image}
    {#if editor.mockup.kind !== "none"}
      <MockupFrame
        mockup={editor.mockup}
        radius={editor.frame.radius}
        shadow={shadowCss(editor.frame.shadow)}
        src={editor.image.src}
        alt="Screenshot being edited"
      />
    {:else}
      <img
        class="recast-shot-image"
        src={editor.image.src}
        alt="Screenshot being edited"
        style:border-radius={`${editor.frame.radius}px`}
        style:box-shadow={shadowCss(editor.frame.shadow)}
      />
    {/if}
  {/if}
</div>

<style>
  .recast-shot-stage {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    max-width: 100%;
    max-height: 100%;
    overflow: hidden;
  }

  .recast-shot-image {
    display: block;
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
</style>
