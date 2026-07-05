<script lang="ts">
  // Live caption overlay over the preview. Reads the generated transcript +
  // style from the store and renders the segment active at the playhead. When
  // the style carries an animation it renders word-by-word (chunking + active-
  // word emphasis + entrance); otherwise it renders the whole line, unchanged.
  // Sits inside `previewRectEl`, so `cqh` font sizing tracks the preview size.
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { ensureFontLoaded } from "$lib/fonts/font-options";
  import { outputToOriginal } from "$lib/timeline/time-map";
  import { computeCanvasGeometry } from "$lib/canvas-geometry";
  import { captionHeightFrac, captionTopFrac } from "$lib/captions/layout";
  import {
    activeChunkIndex,
    activeWordIndex,
    chunkWords,
    isStaticAnimation,
    resolveCaptionAnimation,
  } from "$lib/captions/animation";
  import { withAlpha } from "./annotation-draw.logic";

  let { store }: { store: EditorStore } = $props();

  // Fetch + register the selected Google font (idempotent) so the preview
  // renders it — covers picker changes and reloading a saved project.
  $effect(() => {
    ensureFontLoaded(store.captionStyle.fontFamily, store.captionStyle.fontWeight);
  });

  // The playhead is OUTPUT time; the transcript is SOURCE time. Map back through
  // the time map so captions (and per-word timing) stay synced across cuts and
  // per-segment speed changes.
  const nowOrig = $derived(outputToOriginal(store.timeMap, store.currentTime));

  const active = $derived.by(() => {
    const t = store.transcript;
    if (!t || !store.captionStyle.enabled) return null;
    return t.segments.find((s) => nowOrig >= s.start && nowOrig < s.end) ?? null;
  });

  const anim = $derived(resolveCaptionAnimation(store.captionStyle.animation));
  const animated = $derived(!!active && active.words.length > 0 && !isStaticAnimation(anim));

  // The chunk + active word to show, or null when rendering the static line.
  const view = $derived.by(() => {
    if (!active || !animated) return null;
    const runs = chunkWords(active.words, anim);
    const ci = activeChunkIndex(runs, nowOrig);
    const chunk = runs[ci];
    if (!chunk) return null;
    const wi = activeWordIndex(chunk.words, nowOrig, anim.holdGaps);
    // `key` re-mounts the line when the chunk changes, re-running the entrance.
    return { key: `${active.id}:${ci}`, words: chunk.words, wi };
  });

  // The video rect inside the output canvas (with padding + aspect bars around
  // it). Captions are placed relative to it so top/bottom sit in the padding,
  // not over the video — mirrors the Rust ASS generator.
  const box = $derived.by(() => {
    const s = store.captionStyle;
    const m = store.metadata;
    const g =
      m && m.width && m.height
        ? computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect)
        : null;
    const vLeft = g ? g.videoX / g.canvasW : 0;
    const vRight = g ? (g.videoX + g.videoW) / g.canvasW : 1;
    const vTop = g ? g.videoY / g.canvasH : 0;
    const vBottom = g ? (g.videoY + g.videoH) / g.canvasH : 1;
    const cap = captionHeightFrac(s.fontSizePct, s.maxLines);
    const topFrac = captionTopFrac(s.position, s.offsetPct, cap, { top: vTop, bottom: vBottom });
    // `topFrac === null` → centre vertically on the video.
    const vertical =
      topFrac === null
        ? `top: ${((vTop + vBottom) / 2) * 100}%; transform: translateY(-50%);`
        : `top: ${topFrac * 100}%;`;
    return { leftPct: vLeft * 100, widthPct: (vRight - vLeft) * 100, vertical };
  });

  // Shared text styles for the line element (static and animated alike).
  const textStyle = $derived.by(() => {
    const s = store.captionStyle;
    return [
      `color: ${s.color}`,
      `font-size: ${s.fontSizePct}cqh`,
      `font-family: ${s.fontFamily}`,
      `font-weight: ${s.fontWeight}`,
      `letter-spacing: ${s.letterSpacing}em`,
      `text-transform: ${s.uppercase ? "uppercase" : "none"}`,
      s.outlineWidth > 0
        ? `-webkit-text-stroke: ${s.outlineWidth / 100}em ${s.outlineColor}; paint-order: stroke fill`
        : "",
      s.background === "box" ? `background: ${withAlpha(s.backgroundColor, s.backgroundOpacity / 100)}` : "",
      `--lines: ${s.maxLines}`,
    ]
      .filter(Boolean)
      .join("; ");
  });
</script>

{#if active}
  {@const s = store.captionStyle}
  <div class="caption-layer pointer-events-none absolute inset-0">
    <div
      class="caption-box absolute flex px-[4%]"
      class:justify-start={s.align === "left"}
      class:justify-center={s.align === "center"}
      class:justify-end={s.align === "right"}
      style="left: {box.leftPct}%; width: {box.widthPct}%; {box.vertical}"
    >
    {#if animated && view}
      {#key view.key}
        <span
          class="caption-text leading-tight entrance-{anim.entrance}"
          class:text-left={s.align === "left"}
          class:text-center={s.align === "center"}
          class:text-right={s.align === "right"}
          class:cap-soft={s.background === "soft"}
          class:cap-box={s.background === "box"}
          style="{textStyle}; --entrance-ms: {anim.entranceMs}ms;"
        >
          {#each view.words as word, i (i)}
            {#if i > 0}{" "}{/if}<span
              class="word"
              class:em-scale={anim.emphasis === "scale" && i === view.wi && view.words.length > 1}
              style={anim.emphasis === "color" && i === view.wi
                ? `color: ${anim.emphasisColor}`
                : ""}>{word.text}</span
            >
          {/each}
        </span>
      {/key}
    {:else}
      <span
        class="caption-text leading-tight"
        class:text-left={s.align === "left"}
        class:text-center={s.align === "center"}
        class:text-right={s.align === "right"}
        class:cap-soft={s.background === "soft"}
        class:cap-box={s.background === "box"}
        style={textStyle}
      >
        {active.text}
      </span>
    {/if}
    </div>
  </div>
{/if}

<style>
  /* Establish a size container so the text's `cqh` font scales with the
     preview rectangle (which this layer fills). */
  .caption-layer {
    container-type: size;
  }
  .caption-text {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines, 2);
    line-clamp: var(--lines, 2);
    overflow: hidden;
    max-width: 92%;
    text-wrap: balance;
  }
  .cap-soft {
    text-shadow:
      0 1px 2px rgba(0, 0, 0, 0.9),
      0 0 6px rgba(0, 0, 0, 0.7);
  }
  .cap-box {
    padding: 0.15em 0.55em;
    border-radius: 0.28em;
  }
  /* Active-word emphasis: a quick, eased transition so the highlight tracks
     speech without snapping. Colour emphasis is applied inline (dynamic hex). */
  .word {
    transition:
      color 120ms ease,
      transform 120ms ease;
  }
  .em-scale {
    display: inline-block;
    transform: scale(1.14);
  }
  /* Per-chunk entrance — the keyed line re-mounts on chunk change, re-running
     these. `none` has no rule (renders instantly). */
  .entrance-fade {
    animation: cap-fade var(--entrance-ms, 220ms) ease-out both;
  }
  .entrance-pop {
    animation: cap-pop var(--entrance-ms, 220ms) cubic-bezier(0.2, 0.9, 0.3, 1.3) both;
  }
  .entrance-slide {
    animation: cap-slide var(--entrance-ms, 220ms) ease-out both;
  }
  @keyframes cap-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes cap-pop {
    from {
      opacity: 0;
      transform: scale(0.6);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  @keyframes cap-slide {
    from {
      opacity: 0;
      transform: translateY(0.35em);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .entrance-fade,
    .entrance-pop,
    .entrance-slide {
      animation: none;
    }
    .word {
      transition: none;
    }
  }
</style>
