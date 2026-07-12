<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface EditorStageProps {
    editor: ScreenshotEditorState;
    /** The exportable node — bound out so the toolbar can snapshot it. */
    stageEl?: HTMLElement | null;
  }
</script>

<script lang="ts">
  import MockupFrame from "./MockupFrame.svelte";
  import OverlayLayer from "./OverlayLayer.svelte";
  import { borderCss, shadowCss, transformCss } from "../render";
  import { propsAtTime, propsToTransform } from "../animation";

  let { editor, stageEl = $bindable(null) }: EditorStageProps = $props();

  // When an animation is selected, its interpolated properties drive the
  // framed content (overriding the static 3D controls); else the static look.
  const anim = $derived(
    editor.animationPreset ? propsAtTime(editor.animationPreset, editor.playhead) : null,
  );
  const perspective = $derived(anim ? anim.perspective : editor.transform.perspective);
  const framedTransform = $derived(anim ? propsToTransform(anim) : transformCss(editor.transform));
  const framedOpacity = $derived(anim ? anim.opacity : 1);

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

  const shadow = $derived(shadowCss(editor.shadow));
  const border = $derived(borderCss(editor.frame.border));
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
    <div class="recast-shot-persp" style:perspective={`${perspective}px`}>
      <div
        class="recast-shot-tilt"
        style:transform={framedTransform}
        style:opacity={framedOpacity}
        style:transition={anim ? "none" : undefined}
      >
        {#if editor.mockup.kind !== "none"}
          <MockupFrame
            mockup={editor.mockup}
            radius={editor.frame.radius}
            {shadow}
            {border}
            src={editor.image.src}
            alt="Screenshot being edited"
          />
        {:else}
          <img
            class="recast-shot-image"
            src={editor.image.src}
            alt="Screenshot being edited"
            style:border-radius={`${editor.frame.radius}px`}
            style:box-shadow={shadow}
            style:border={border}
          />
        {/if}
      </div>
    </div>
    <OverlayLayer {editor} />
  {/if}
</div>

<style>
  .recast-shot-stage {
    position: relative;
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

  /* Perspective wrapper so the framed content can tilt in 3D; both layers fill
     the padded stage so a mockup's 100% sizing still resolves. */
  .recast-shot-persp {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  .recast-shot-tilt {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    transform-style: preserve-3d;
    transition: transform 120ms ease-out;
  }
</style>
