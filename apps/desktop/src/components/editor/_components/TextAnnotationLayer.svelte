<script lang="ts">
  import { evalOpacity, evalZoom } from "$lib/annotations/eval";
  import { canvasToUV, uvToCanvas, videoRectPx } from "$lib/annotations/uv";
  import { FRAME_ANCHORS, snap, type SnapAnchor } from "$lib/annotations/snap";
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

  // Drag state for text annotations. Text is a sibling HTML element so we
  // can't piggyback on AnnotationOverlay's pointer flow — we run our own
  // here, using the same UV math + snap engine so behavior matches.
  type TextDrag = {
    id: string;
    startX: number; // UV
    startY: number;
    pointerStartUV: { x: number; y: number };
    moved: boolean; // true once we cross the click vs drag threshold
  } | null;
  let drag: TextDrag = $state(null);
  // Click-vs-drag threshold in CSS px. Below this the gesture is treated as a
  // click (select); above it, the gesture commits a move.
  const CLICK_DRAG_THRESHOLD_PX = 3;

  function videoRectCss() {
    return videoRectPx(layerSize.w, layerSize.h, store.metadata, store.padding);
  }

  function uvToCss(ux: number, uy: number, t: number) {
    return uvToCanvas(ux, uy, videoRectCss(), evalZoom(store.zoomRegions, t));
  }

  function pointerToUV(e: PointerEvent, t: number) {
    if (!layerEl) return { x: 0, y: 0 };
    const rect = layerEl.getBoundingClientRect();
    return canvasToUV(
      e.clientX - rect.left,
      e.clientY - rect.top,
      videoRectCss(),
      evalZoom(store.zoomRegions, t),
    );
  }

  function pointerToCss(e: PointerEvent) {
    if (!layerEl) return { x: 0, y: 0 };
    const rect = layerEl.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
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

  /** Build snap anchors from frame edges + every other annotation's box.
   *  Mirrors the anchor set used by AnnotationOverlay for canvas-rendered
   *  shapes so dragging text feels identical to dragging a rectangle. */
  function buildSnapAnchors(excludeId: string | null): SnapAnchor[] {
    const anchors: SnapAnchor[] = [...FRAME_ANCHORS];
    for (const a of store.annotations) {
      if (a.id === excludeId) continue;
      if (a.hidden) continue;
      if (a.kind.kind === "arrow") {
        anchors.push({ axis: "x", value: a.kind.x1 });
        anchors.push({ axis: "y", value: a.kind.y1 });
        anchors.push({ axis: "x", value: a.kind.x2 });
        anchors.push({ axis: "y", value: a.kind.y2 });
        continue;
      }
      const k = a.kind;
      if (
        k.kind === "rect" ||
        k.kind === "ellipse" ||
        k.kind === "image" ||
        k.kind === "text"
      ) {
        const x = Math.min(k.x, k.x + k.w);
        const y = Math.min(k.y, k.y + k.h);
        const w = Math.abs(k.w);
        const h = Math.abs(k.h);
        anchors.push({ axis: "x", value: x });
        anchors.push({ axis: "x", value: x + w / 2 });
        anchors.push({ axis: "x", value: x + w });
        anchors.push({ axis: "y", value: y });
        anchors.push({ axis: "y", value: y + h / 2 });
        anchors.push({ axis: "y", value: y + h });
      }
    }
    return anchors;
  }

  function handleTextPointerDown(e: PointerEvent, a: Annotation) {
    if (editingId === a.id) return; // let contenteditable take the gesture
    if (a.locked || a.kind.kind !== "text") return;
    if (e.button !== 0) return;
    // Don't fight the WebGL canvas / focus overlay — text dragging is only
    // available on the Annotations tab (matches the rest of the editor's
    // pointer gating).
    if (store.activePanel !== "annotations") return;

    e.stopPropagation();
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);

    const t = playbackTime();
    const pointerUV = pointerToUV(e, t);
    const startCss = pointerToCss(e);

    drag = {
      id: a.id,
      startX: a.kind.x,
      startY: a.kind.y,
      pointerStartUV: pointerUV,
      moved: false,
    };
    // Selecting on press matches Figma/Keynote — the rest of the panel
    // updates immediately even before the user commits to a drag.
    store.selectedAnnotationId = a.id;
    store.pushUndoState();

    // Stash the press position on the element for the threshold check.
    target.dataset.dragStartX = String(startCss.x);
    target.dataset.dragStartY = String(startCss.y);
  }

  function handleTextPointerMove(e: PointerEvent, a: Annotation) {
    if (!drag || drag.id !== a.id) return;
    if (a.kind.kind !== "text") return;

    const t = playbackTime();
    const css = pointerToCss(e);
    const target = e.currentTarget as HTMLElement;
    const startX = +(target.dataset.dragStartX ?? "0");
    const startY = +(target.dataset.dragStartY ?? "0");
    const moved =
      Math.hypot(css.x - startX, css.y - startY) >= CLICK_DRAG_THRESHOLD_PX;
    if (!moved && !drag.moved) return;
    drag.moved = true;

    const rawUv = pointerToUV(e, t);
    const dx = rawUv.x - drag.pointerStartUV.x;
    const dy = rawUv.y - drag.pointerStartUV.y;
    let nx = drag.startX + dx;
    let ny = drag.startY + dy;

    // Snap (Alt held bypasses, matching the canvas overlay).
    if (!e.altKey && store.annotationSnapEnabled) {
      const anchors = buildSnapAnchors(drag.id);
      const result = snap(nx, ny, anchors, 0.005, true);
      nx = result.x;
      ny = result.y;
    }

    store.updateAnnotation(a.id, {
      kind: { ...a.kind, x: nx, y: ny },
    });
  }

  function handleTextPointerUp(e: PointerEvent, a: Annotation) {
    const target = e.currentTarget as HTMLElement;
    try {
      target.releasePointerCapture(e.pointerId);
    } catch {
      // capture may have already been released by the browser — ignore.
    }
    delete target.dataset.dragStartX;
    delete target.dataset.dragStartY;
    drag = null;
    void a;
  }
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
      {@const isDragging = drag?.id === a.id && drag?.moved}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        data-text-anno-id={a.id}
        class="absolute origin-top-left select-none whitespace-pre-wrap wrap-break-word"
        class:outline={isSelected && isActiveTab}
        class:outline-1={isSelected && isActiveTab}
        class:outline-dashed={isSelected && isActiveTab && !isEditing}
        class:outline-blue-500={isSelected && isActiveTab}
        class:cursor-text={isEditing}
        class:cursor-grab={interactive && !isEditing && !isDragging}
        class:cursor-grabbing={isDragging}
        contenteditable={isEditing}
        style={styleFor(a)}
        onpointerdown={(e) => handleTextPointerDown(e, a)}
        onpointermove={(e) => handleTextPointerMove(e, a)}
        onpointerup={(e) => handleTextPointerUp(e, a)}
        onpointercancel={(e) => handleTextPointerUp(e, a)}
        ondblclick={(e) => {
          if (!interactive) return;
          e.stopPropagation();
          startEditing(a);
        }}
        onclick={(e) => {
          if (!interactive) return;
          if (isEditing) return;
          // Suppress the click that tails a successful drag — otherwise the
          // selection would be reasserted but the gesture would also re-emit
          // a stray select. The drag is already over by `click` time.
          if (drag?.id === a.id && drag?.moved) {
            e.stopPropagation();
            return;
          }
          e.stopPropagation();
          store.selectedAnnotationId = a.id;
        }}
        onblur={(e) => commitEditing(a, e.currentTarget as HTMLElement)}
        onkeydown={(e) => handleKeyDown(e, a)}
        style:pointer-events={interactive ? "auto" : "none"}
        style:touch-action={interactive ? "none" : "auto"}
      >{a.kind.content}</div>
    {/if}
  {/each}
</div>
