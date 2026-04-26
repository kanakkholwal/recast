<script lang="ts">
  import { bezierY } from "$lib/easing/cubic-bezier";
  import type {
    Annotation,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { framePaddingPixels } from "$lib/stores/editor-store.svelte";
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

  function getDpr(): number {
    return window.devicePixelRatio || 1;
  }

  function compW(): number {
    const meta = store.metadata;
    if (!meta) return 0;
    const paddingPx = framePaddingPixels(store.padding, meta);
    return meta.width + paddingPx * 2;
  }

  /** CSS-px rect (relative to layerEl) of the video region. */
  function videoRectCss(): { x: number; y: number; w: number; h: number } {
    const w = layerSize.w;
    const h = layerSize.h;
    const total = compW();
    const meta = store.metadata;
    const sourcePaddingPx = meta ? framePaddingPixels(store.padding, meta) : 0;
    const padPx = total > 0 ? (sourcePaddingPx / total) * w : 0;
    return { x: padPx, y: padPx, w: w - 2 * padPx, h: h - 2 * padPx };
  }

  /** Mirror of AnnotationOverlay.evalZoom — kept in sync deliberately. */
  function evalZoom(t: number): { scale: number; cx: number; cy: number } {
    for (const r of store.zoomRegions) {
      if (t <= r.start || t >= r.end) continue;
      const duration = Math.max(0, r.end - r.start);
      const half = duration * 0.5;
      const rampIn = Math.min(Math.max(0, r.rampIn), half);
      const rampOut = Math.min(Math.max(0, r.rampOut), half);
      const holdStart = r.start + rampIn;
      const holdEnd = r.end - rampOut;
      const cxTarget = r.centerX ?? 0.5;
      const cyTarget = r.centerY ?? 0.5;
      let phase: number;
      let curve;
      let atHold = false;
      if (t < holdStart) {
        phase = rampIn > 0 ? (t - r.start) / rampIn : 1;
        curve = r.easeIn;
      } else if (t > holdEnd) {
        phase = rampOut > 0 ? (r.end - t) / rampOut : 1;
        curve = r.easeOut;
      } else {
        atHold = true;
        phase = 1;
        curve = r.easeIn;
      }
      phase = Math.max(0, Math.min(1, phase));
      const eased = atHold ? 1 : bezierY(curve, phase);
      return {
        scale: 1 + (r.scale - 1) * eased,
        cx: 0.5 + (cxTarget - 0.5) * eased,
        cy: 0.5 + (cyTarget - 0.5) * eased,
      };
    }
    return { scale: 1, cx: 0.5, cy: 0.5 };
  }

  function evalOpacity(a: Annotation, t: number): number {
    if (t <= a.start || t >= a.end) return 0;
    const dur = Math.max(0, a.end - a.start);
    const half = dur * 0.5;
    const rampIn = Math.min(Math.max(0, a.rampIn), half);
    const rampOut = Math.min(Math.max(0, a.rampOut), half);
    const holdStart = a.start + rampIn;
    const holdEnd = a.end - rampOut;
    let phase: number;
    let curve;
    if (t < holdStart) {
      phase = rampIn > 0 ? (t - a.start) / rampIn : 1;
      curve = a.easeIn;
    } else if (t > holdEnd) {
      phase = rampOut > 0 ? (a.end - t) / rampOut : 1;
      curve = a.easeOut;
    } else {
      return 1;
    }
    phase = Math.max(0, Math.min(1, phase));
    return Math.max(0, Math.min(1, bezierY(curve, phase)));
  }

  function uvToCss(ux: number, uy: number, t: number): { x: number; y: number } {
    const rect = videoRectCss();
    const zoom = evalZoom(t);
    const preX = (ux - zoom.cx) * zoom.scale + zoom.cx;
    const preY = (uy - zoom.cy) * zoom.scale + zoom.cy;
    return {
      x: rect.x + preX * rect.w,
      y: rect.y + preY * rect.h,
    };
  }

  function playbackTime(): number {
    return videoEl?.currentTime ?? store.currentTime;
  }

  // Recompute size and request a redraw on every animation frame. This is
  // cheap because we only emit DOM updates when annotation timings, video
  // playback, or layer dimensions actually move; Svelte's reactivity handles
  // diffing.
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
    return [
      `left: ${tl.x}px`,
      `top: ${tl.y}px`,
      `width: ${cssW}px`,
      `min-height: ${cssH}px`,
      `opacity: ${opacity}`,
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

  function handleKeyDown(e: KeyboardEvent, a: Annotation) {
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
>
  {#each store.annotations as a (a.id)}
    {#if a.kind.kind === "text"}
      {@const isEditing = editingId === a.id}
      {@const isSelected = a.id === store.selectedAnnotationId}
      {@const isActiveTab = store.activePanel === "annotations"}
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
          if (!isActiveTab) return;
          e.stopPropagation();
          startEditing(a);
        }}
        onclick={(e) => {
          if (!isActiveTab) return;
          if (isEditing) return;
          e.stopPropagation();
          store.selectedAnnotationId = a.id;
        }}
        onblur={(e) => commitEditing(a, e.currentTarget as HTMLElement)}
        onkeydown={(e) => handleKeyDown(e, a)}
        style:pointer-events={isActiveTab ? (isEditing ? "auto" : "auto") : "none"}
      >{a.kind.content}</div>
    {/if}
  {/each}
</div>
