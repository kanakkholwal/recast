<script lang="ts">
  import { evalOpacity, evalZoom } from "$lib/annotations/eval";
  import { normaliseBox, uvToCanvas, videoRectPx } from "$lib/annotations/uv";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { onDestroy, onMount } from "svelte";

  // HTML layer for blur annotations. Sits between the WebGL composite and
  // the 2D AnnotationOverlay canvas so the canvas can still draw selection
  // handles ON TOP of the blur, while the blur itself runs as native
  // `backdrop-filter: blur(Npx)` against the live video pixels underneath.
  //
  // The 2D canvas can't backdrop-blur its own pixels (and Canvas2D's `filter`
  // only blurs what you draw, not what's behind it), so we delegate the
  // visual to compositor-accelerated CSS. At export time, FFmpeg's `boxblur`
  // filter (see `commands/ffmpeg.rs::build_annotation_blur_complex`) does
  // the equivalent in a deterministic offline pass.

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    /** Container that wraps the WebGL preview canvas — we stretch to fit. */
    targetEl: HTMLElement | null;
  }

  let { store, videoEl, targetEl }: Props = $props();

  let layerEl: HTMLDivElement | undefined = $state();
  let layerSize = $state({ w: 0, h: 0 });
  let resizeObserver: ResizeObserver | null = null;
  let rafHandle: number | null = null;
  // rAF tick so positions track playback (the store doesn't dispatch on every
  // video frame). Same pattern as TextAnnotationLayer.
  let _frame = $state(0);

  function videoRectCss() {
    return videoRectPx(layerSize.w, layerSize.h, store.metadata, store.padding);
  }

  function uvToCss(ux: number, uy: number, t: number) {
    return uvToCanvas(ux, uy, videoRectCss(), evalZoom(store.zoomRegions, t));
  }

  function playbackTime(): number {
    return videoEl?.currentTime ?? store.currentTime;
  }

  function rafTick() {
    if (layerEl) {
      const r = layerEl.getBoundingClientRect();
      if (r.width !== layerSize.w || r.height !== layerSize.h) {
        layerSize = { w: r.width, h: r.height };
      }
    }
    _frame++;
    rafHandle = requestAnimationFrame(rafTick);
  }

  onMount(() => {
    rafHandle = requestAnimationFrame(rafTick);
    if (targetEl) {
      resizeObserver = new ResizeObserver(() => {
        if (layerEl) {
          const r = layerEl.getBoundingClientRect();
          layerSize = { w: r.width, h: r.height };
        }
      });
      resizeObserver.observe(targetEl);
    }
  });
  onDestroy(() => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle);
    resizeObserver?.disconnect();
  });

  /**
   * Build the per-annotation positioning + tint style. Strength 0..1 maps to
   * 0..32 px of `backdrop-filter: blur(Npx)`, which mirrors the export-side
   * boxblur radius cap of 5% of the shorter canvas edge — the two pipelines
   * stay close enough that what users see in preview matches what they get
   * in the exported file.
   */
  function styleFor(a: Annotation): string {
    if (a.kind.kind !== "blur") return "display: none;";
    void _frame;
    const t = playbackTime();
    const opacity = evalOpacity(a, t);
    if (opacity <= 0) return "display: none;";

    const k = a.kind;
    const box = normaliseBox(k);
    const tl = uvToCss(box.x, box.y, t);
    const br = uvToCss(box.x + box.w, box.y + box.h, t);
    const x = Math.min(tl.x, br.x);
    const y = Math.min(tl.y, br.y);
    const w = Math.abs(br.x - tl.x);
    const h = Math.abs(br.y - tl.y);
    if (w < 1 || h < 1) return "display: none;";

    // 0..1 → 0..32 px. The slider's mental model: 50% strength ≈ 16px blur,
    // which reads as "obviously obscured" against most UI captures.
    const blurPx = Math.max(0, Math.min(32, k.strength * 32));
    const radiusPx = Math.max(
      0,
      k.radius * Math.min(layerSize.w, layerSize.h),
    );

    // Variant tint: layered on top of the backdrop-blur so it reads as a
    // semantic privacy treatment, not just a soft smudge.
    let tint = "transparent";
    if (k.variant === "white") tint = "rgba(255,255,255,0.30)";
    else if (k.variant === "black") tint = "rgba(0,0,0,0.30)";
    else if (k.variant === "color") tint = hexToRgba(k.tintColor, 0.30);
    // glass = no tint, the blur alone reads.

    const filter = `blur(${blurPx.toFixed(2)}px)`;
    return [
      "position: absolute",
      `left: ${x.toFixed(2)}px`,
      `top: ${y.toFixed(2)}px`,
      `width: ${w.toFixed(2)}px`,
      `height: ${h.toFixed(2)}px`,
      `border-radius: ${radiusPx.toFixed(2)}px`,
      `background: ${tint}`,
      `backdrop-filter: ${filter}`,
      `-webkit-backdrop-filter: ${filter}`,
      `opacity: ${opacity.toFixed(3)}`,
      `pointer-events: none`,
      // Hint compositor for cheaper redraws while dragging.
      `will-change: filter, transform`,
      `overflow: hidden`,
    ].join(";");
  }

  function hexToRgba(hex: string, alpha: number): string {
    const m = /^#?([0-9a-fA-F]{6})$/.exec(hex.trim());
    if (!m) return `rgba(0,0,0,${alpha})`;
    const v = parseInt(m[1], 16);
    return `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},${alpha})`;
  }

  // Only blur annotations that aren't hidden + are inside their visibility
  // window — derived in render so unmounted divs don't ghost layout.
  const blurs = $derived(
    store.annotations.filter((a) => a.kind.kind === "blur" && !a.hidden),
  );
</script>

<div
  bind:this={layerEl}
  class="pointer-events-none absolute inset-0 overflow-hidden"
  aria-hidden="true"
>
  {#each blurs as a (a.id)}
    <div style={styleFor(a)} data-blur-id={a.id}></div>
  {/each}
</div>
