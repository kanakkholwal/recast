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
  import { borderCss, filtersCss, shadowCss, styleFrameBackground, transformCss } from "../render";
  import { propsAtTime, propsToTransform } from "../animation";

  let { editor, stageEl = $bindable(null) }: EditorStageProps = $props();

  // When an animation is active, its interpolated properties drive the framed
  // content (overriding the static 3D controls); else the static look. In
  // keyframe mode the stage stays editable (static) while paused, so the user
  // can set a look and capture it; playback still previews the animation.
  const anim = $derived.by(() => {
    const preset = editor.activePreset;
    if (!preset) return null;
    if (editor.keyframeMode && !editor.playing) return null;
    return propsAtTime(preset, editor.animationTime);
  });
  const perspective = $derived(anim ? anim.perspective : editor.transform.perspective);
  // The static/animated 3D transform, then the user's image-size multiplier so
  // "Scale" reads as resizing the shot within the padded stage.
  const framedTransform = $derived(
    `${anim ? propsToTransform(anim) : transformCss(editor.transform)} scale(${editor.imageScale / 100})`,
  );
  // Animation opacity composes with the persistent image opacity for a PRESET
  // (the preset animates a 0..1 fade on top of the user's opacity). In keyframe
  // mode the captured opacity already IS the user's imageOpacity, so use it
  // directly to avoid squaring it.
  const framedOpacity = $derived(
    anim
      ? editor.keyframeMode
        ? anim.opacity
        : anim.opacity * editor.imageOpacity
      : editor.imageOpacity,
  );
  const imageFilter = $derived(filtersCss(editor.filters));

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

  // Ruler tick positions (px from the origin) at the configured interval. Capped
  // so a tiny interval can't render thousands of labels; extras clip.
  const rulerTicks = $derived.by(() => {
    const step = Math.max(10, editor.rulerInterval);
    const ticks: number[] = [];
    for (let x = step; x <= step * 40; x += step) ticks.push(x);
    return ticks;
  });

  // Style-frame wrapper (glass/outline/solid card around the shot). When active,
  // the drop shadow moves to the wrapper so it hugs the card, not the raw image.
  const styleActive = $derived(
    editor.imageStyle.preset !== "default" && editor.mockup.kind === "none",
  );
  const styleBg = $derived(styleFrameBackground(editor.imageStyle));
  // Concentric outer radius: inner radius plus a touch, or square when unrounded.
  const styleRadius = $derived(editor.frame.radius > 0 ? editor.frame.radius + 6 : 0);
</script>

<!-- The stage IS the export node: what renders here is what gets snapshotted. -->
<div
  bind:this={stageEl}
  class="recast-shot-stage"
  class:recast-shot-checkerboard={editor.background.kind === "transparent"}
  style:aspect-ratio={`${aspectRatio}`}
  style:padding={`${editor.frame.padding}%`}
  style:border-radius={editor.canvasRadius > 0 ? `${editor.canvasRadius}px` : undefined}
>
  <!-- Backdrop as its own layer so blur applies to the paint only, never the
       screenshot. Skipped for transparent (the checkerboard shows through). -->
  {#if editor.background.kind !== "transparent"}
    <div
      class="recast-shot-bg"
      style:background={backgroundCss}
      style:filter={editor.backgroundBlur > 0 ? `blur(${editor.backgroundBlur}px)` : undefined}
    ></div>
  {/if}
  {#if editor.backgroundNoise > 0}
    <!-- Softer curve: cap effective opacity ~0.5 so grain stays subtle (the
         reference scales the noise variance, not just the layer opacity). -->
    <div class="recast-shot-noise" style:opacity={(editor.backgroundNoise / 100) * 0.5}></div>
  {/if}

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
            filter={imageFilter}
            src={editor.image.src}
            alt="Screenshot being edited"
          />
        {:else if styleActive}
          <!-- Style-frame card: the wrapper carries background + padding +
               shadow; the shot sits inside at its own corner radius. -->
          <div
            class="recast-shot-frame"
            style:background={styleBg}
            style:padding={`${editor.imageStyle.padding}%`}
            style:border-radius={`${styleRadius}px`}
            style:box-shadow={shadow}
          >
            <img
              class="recast-shot-image"
              src={editor.image.src}
              alt="Screenshot being edited"
              style:border-radius={`${editor.frame.radius}px`}
              style:filter={imageFilter === "none" ? undefined : imageFilter}
            />
          </div>
        {:else}
          <img
            class="recast-shot-image"
            src={editor.image.src}
            alt="Screenshot being edited"
            style:border-radius={`${editor.frame.radius}px`}
            style:box-shadow={shadow}
            style:border={border}
            style:filter={imageFilter === "none" ? undefined : imageFilter}
          />
        {/if}
      </div>
    </div>
    <OverlayLayer {editor} />
  {/if}

  <!-- Editing guides. `data-export-ignore` keeps them out of every snapshot
       (still, image and video), so they are preview-only by construction. -->
  {#if editor.showGrid}
    <div
      class="recast-shot-grid"
      data-export-ignore
      style:background-size={`${editor.gridSize}px ${editor.gridSize}px`}
    ></div>
  {/if}
  {#if editor.showRulers}
    <div class="recast-shot-rulers" data-export-ignore>
      <div class="ruler-h" style:background-size={`${editor.rulerInterval}px 100%`}>
        {#each rulerTicks as tick (tick)}
          <span class="ruler-label" style:left={`${tick}px`}>{tick}</span>
        {/each}
      </div>
      <div class="ruler-v" style:background-size={`100% ${editor.rulerInterval}px`}>
        {#each rulerTicks as tick (tick)}
          <span class="ruler-label ruler-label-v" style:top={`${tick}px`}>{tick}</span>
        {/each}
      </div>
      <div class="ruler-corner"></div>
    </div>
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

  /* Style-frame card hugs the shot; box-sizing keeps the % padding even. */
  .recast-shot-frame {
    display: flex;
    align-items: center;
    justify-content: center;
    max-width: 100%;
    max-height: 100%;
    box-sizing: border-box;
  }
  .recast-shot-frame .recast-shot-image {
    /* Percent padding on the wrapper already reserved space; let the image use
       what remains without its own max clamp fighting the wrapper. */
    max-width: 100%;
    max-height: 100%;
  }

  /* Backdrop paint layer. Slightly over-inset so an applied blur doesn't reveal
     hard edges at the stage border. */
  .recast-shot-bg {
    position: absolute;
    inset: -8%;
    z-index: 0;
  }

  /* Procedural grain, tinted by the layer's opacity. Inline SVG keeps it
     self-contained and export-safe (no external asset). */
  .recast-shot-noise {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
  }

  /* Guides sit above everything but are stripped from exports. */
  .recast-shot-grid {
    position: absolute;
    inset: 0;
    z-index: 5;
    pointer-events: none;
    background-image:
      linear-gradient(to right, rgba(127, 127, 127, 0.28) 1px, transparent 1px),
      linear-gradient(to bottom, rgba(127, 127, 127, 0.28) 1px, transparent 1px);
  }

  .recast-shot-rulers {
    position: absolute;
    inset: 0;
    z-index: 6;
    pointer-events: none;
  }
  .recast-shot-rulers .ruler-h,
  .recast-shot-rulers .ruler-v {
    position: absolute;
    background-color: rgba(20, 20, 22, 0.55);
    background-repeat: repeat;
    overflow: hidden;
  }
  .recast-shot-rulers .ruler-h {
    top: 0;
    left: 0;
    right: 0;
    height: 16px;
    background-image: linear-gradient(
      to right,
      rgba(255, 255, 255, 0.75) 1px,
      transparent 1px
    );
  }
  .recast-shot-rulers .ruler-v {
    top: 0;
    bottom: 0;
    left: 0;
    width: 16px;
    background-image: linear-gradient(
      to bottom,
      rgba(255, 255, 255, 0.75) 1px,
      transparent 1px
    );
  }
  .recast-shot-rulers .ruler-label {
    position: absolute;
    font-size: 8px;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.7);
    pointer-events: none;
  }
  .recast-shot-rulers .ruler-h .ruler-label {
    top: 3px;
    transform: translateX(2px);
  }
  .recast-shot-rulers .ruler-v .ruler-label {
    left: 2px;
    transform: translateY(2px);
  }
  .recast-shot-rulers .ruler-corner {
    position: absolute;
    top: 0;
    left: 0;
    width: 16px;
    height: 16px;
    background: rgba(20, 20, 22, 0.8);
    z-index: 1;
  }

  /* Perspective wrapper so the framed content can tilt in 3D; both layers fill
     the padded stage so a mockup's 100% sizing still resolves. Sits above the
     backdrop/noise layers. */
  .recast-shot-persp {
    position: relative;
    z-index: 1;
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
