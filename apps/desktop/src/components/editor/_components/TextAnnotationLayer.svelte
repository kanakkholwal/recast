<script lang="ts">
  import { evalOpacity, evalZoom } from "$lib/annotations/eval";
  import { uvToCanvas, videoRectPx } from "$lib/annotations/uv";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { onDestroy, onMount, tick } from "svelte";

  // HTML layer for text annotations only. Lives as a sibling to
  // AnnotationOverlay (the 2D canvas) so that:
  //   - text uses the WebView's full glyph rendering (kerning, ligatures)
  //   - inline editing via `contenteditable` is trivial
  //   - inert canvas-side draw code stays simple (just a `kind === "text"`
  //     skip)
  //
  // At export time, `lib/export/rasterize-text.ts` walks every text annotation
  // and rasterizes it to a PNG that's then fed to the Rust pipeline as an
  // image-kind annotation. Rust never sees fonts.

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    /** The container that wraps the WebGL preview canvas — we stretch to fit. */
    targetEl: HTMLElement | null;
  }

  let { store, videoEl, targetEl }: Props = $props();

  let layerEl: HTMLDivElement | undefined = $state();
  let layerSize = $state({ w: 0, h: 0 });
  let editingId = $state<string | null>(null);
  let resizeObserver: ResizeObserver | null = null;
  let rafHandle: number | null = null;
  // Track time/zoom changes via rAF; the store doesn't fire on every video
  // tick so the cheapest correct path is to rebuild positions per frame.
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

  function tick_() {
    if (layerEl) {
      const r = layerEl.getBoundingClientRect();
      if (r.width !== layerSize.w || r.height !== layerSize.h) {
        layerSize = { w: r.width, h: r.height };
      }
    }
    _frame++;
    rafHandle = requestAnimationFrame(tick_);
  }

  onMount(() => {
    rafHandle = requestAnimationFrame(tick_);
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

  // Per-annotation reactive style derived from the box, the playhead, and
  // the layer dims. The `_frame` dependency forces re-derive on rAF ticks
  // so the position tracks playback / zoom animations.
  function styleFor(a: Annotation): string {
    if (a.kind.kind !== "text") return "";
    void _frame;
    const t = playbackTime();
    const opacity = evalOpacity(a, t);
    const k = a.kind;
    const x = Math.min(k.x, k.x + k.w);
    const y = Math.min(k.y, k.y + k.h);
    const w = Math.abs(k.w);
    const h = Math.abs(k.h);
    const tl = uvToCss(x, y, t);
    const br = uvToCss(x + w, y + h, t);
    const cssW = Math.max(0, br.x - tl.x);
    const cssH = Math.max(0, br.y - tl.y);
    const fontSizePx = k.fontSize * layerSize.h;
    const z = a.zIndex ?? 0;
    return [
      `left: ${tl.x}px`,
      `top: ${tl.y}px`,
      `width: ${cssW}px`,
      `min-height: ${cssH}px`,
      `opacity: ${opacity}`,
      `z-index: ${z}`,
      `font-family: ${k.fontFamily}`,
      `font-size: ${fontSizePx}px`,
      `font-weight: ${k.fontWeight}`,
      `color: ${k.color}`,
      `text-align: ${k.align}`,
      `line-height: ${k.lineHeight}`,
    ].join(";");
  }

  function startEditing(a: Annotation) {
    if (a.kind.kind !== "text") return;
    if (a.locked) return;
    store.pushUndoState();
    editingId = a.id;
    void tick().then(() => {
      const el = document.querySelector(
        `[data-text-anno-id="${a.id}"]`,
      ) as HTMLElement | null;
      if (el) {
        el.focus();
        // Select all on entry — Keynote behaviour.
        const range = document.createRange();
        range.selectNodeContents(el);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    });
  }

  function commitEditing(a: Annotation, el: HTMLElement) {
    if (a.kind.kind !== "text") return;
    const content = el.innerText.replace(/​/g, "");
    if (a.kind.content !== content) {
      store.updateAnnotation(a.id, {
        kind: { ...a.kind, content },
      });
    }
    editingId = null;
  }

  function handleKeyDown(e: KeyboardEvent, _a: Annotation) {
    if (e.key === "Escape") {
      e.preventDefault();
      const el = e.currentTarget as HTMLElement;
      el.blur();
    }
  }

  // Pointer interactions: the layer doesn't itself drag boxes — that stays
  // on the canvas overlay. We only handle the click-to-select and
  // double-click-to-edit affordances. Move/resize of text falls back to the
  // canvas overlay's "body" hit-test when the user clicks on the text rect
  // outside edit mode (forwarded by setting pointer-events to "none" on the
  // text element while not editing).
</script>

<div
  bind:this={layerEl}
  class="pointer-events-none absolute inset-0 overflow-hidden"
  class:hidden={store.annotationsGloballyHidden}
>
  {#each store.annotationsByZ as a (a.id)}
    {#if a.kind.kind === "text" && !a.hidden}
      {@const isEditing = editingId === a.id}
      {@const isSelected = a.id === store.selectedAnnotationId}
      {@const isActiveTab = store.activePanel === "annotations"}
      {@const interactive = isActiveTab && !a.locked}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        data-text-anno-id={a.id}
        class="absolute origin-top-left whitespace-pre-wrap wrap-break-word"
        class:outline={isSelected && isActiveTab}
        class:outline-1={isSelected && isActiveTab}
        class:outline-dashed={isSelected && isActiveTab && !isEditing}
        class:outline-blue-500={isSelected && isActiveTab}
        class:cursor-text={isEditing}
        contenteditable={isEditing}
        style={styleFor(a)}
        ondblclick={(e) => {
          if (!interactive) return;
          e.stopPropagation();
          startEditing(a);
        }}
        onclick={(e) => {
          if (!interactive) return;
          if (isEditing) return;
          e.stopPropagation();
          store.selectedAnnotationId = a.id;
        }}
        onblur={(e) => commitEditing(a, e.currentTarget as HTMLElement)}
        onkeydown={(e) => handleKeyDown(e, a)}
        style:pointer-events={interactive ? "auto" : "none"}
      >{a.kind.content}</div>
    {/if}
  {/each}
</div>
