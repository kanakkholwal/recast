<script lang="ts">
  import {
    evalOpacity,
    evalZoom,
    type ZoomRegionLike,
  } from "$lib/annotations/eval";
  import {
    handlePositions,
    hitTestAnnotation,
    hitTestHandle,
    pointToSegmentDist,
    type HandleName,
  } from "$lib/annotations/hit";
  import {
    canvasToUV,
    normaliseBox,
    uvToCanvas,
    videoRectPx,
    type Rect,
  } from "$lib/annotations/uv";
  import { FRAME_ANCHORS, snap, type SnapAnchor } from "$lib/annotations/snap";
  import type {
    Annotation,
    AnnotationKind,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { onDestroy, onMount } from "svelte";

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    /** The container that wraps the WebGL preview canvas — we stretch to fit. */
    targetEl: HTMLElement | null;
    /** The WebGL composite canvas. Used as the source for blur annotations,
     *  so we can blur the actual rendered frame (background + padding +
     *  shadow + video) rather than just the bare video. */
    compositeCanvasEl?: HTMLCanvasElement | null;
  }

  let { store, videoEl, targetEl, compositeCanvasEl = null }: Props = $props();

  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rafHandle: number | null = null;
  let resizeObserver: ResizeObserver | null = null;

  //  Drag / placement state
  type DragState =
    | null
    | {
        kind: "move";
        id: string;
        startX: number; // UV (top-left for boxes; x1 for arrows)
        startY: number;
        // For arrows, also keep the second endpoint so we can move both
        // together while preserving the arrow's orientation/length.
        startX2?: number;
        startY2?: number;
        pointerStartUV: { x: number; y: number };
      }
    | {
        kind: "resize";
        id: string;
        handle: HandleName;
        startBox: { x: number; y: number; w: number; h: number };
      }
    | {
        kind: "place";
        id: string;
        anchor: { x: number; y: number };
      };
  let drag: DragState = null;
  // Active snap guides for the current drag, in UV space. Cleared on
  // pointerup. Capped to 4 simultaneous guides to avoid visual noise.
  let snapGuides: SnapAnchor[] = $state([]);
  // What's under the pointer, used purely for cursor affordance ("grab" on
  // body, "nwse-resize" / "ns-resize" / etc on handles). Cleared on leave.
  let hoverHandle: HandleName | null | "tool" = $state(null);

  const HANDLE_RADIUS_PX = 6; // CSS px half-size of resize handles
  const SELECTION_COLOUR = "#3b82f6";
  const HOVER_FLASH_COLOUR = "rgba(59,130,246,0.85)";
  const SNAP_GUIDE_COLOUR = "rgba(59,130,246,0.7)";

  //  Helpers — thin wrappers around shared modules so this file just owns
  //  rendering + interaction state, not geometry math.

  function getDpr(): number {
    return window.devicePixelRatio || 1;
  }

  function rectPx(): Rect {
    if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
    return videoRectPx(canvasEl.width, canvasEl.height, store.metadata, store.padding);
  }

  function projectUV(ux: number, uy: number, t: number) {
    return uvToCanvas(ux, uy, rectPx(), evalZoom(zoomRegions(), t));
  }

  function unprojectUV(cx: number, cy: number, t: number) {
    return canvasToUV(cx, cy, rectPx(), evalZoom(zoomRegions(), t));
  }

  function zoomRegions(): ZoomRegionLike[] {
    return store.zoomRegions;
  }

  /** True if this annotation should NOT draw on the 2D-canvas overlay. Text
   * lives in a separate HTML layer (TextAnnotationLayer) so the WebView
   * handles glyph rendering and inline edit. */
  function isCanvasDrawn(k: AnnotationKind): boolean {
    return k.kind !== "text";
  }

  function pointerToCanvasPx(e: PointerEvent): { x: number; y: number } {
    if (!canvasEl) return { x: 0, y: 0 };
    const rect = canvasEl.getBoundingClientRect();
    const dpr = getDpr();
    return {
      x: (e.clientX - rect.left) * dpr,
      y: (e.clientY - rect.top) * dpr,
    };
  }

  function playbackTime(): number {
    return videoEl?.currentTime ?? store.currentTime;
  }

  //  Drawing

  function drawAnnotation(
    ctx: CanvasRenderingContext2D,
    a: Annotation,
    opacity: number,
    t: number,
  ) {
    // Blur annotations bypass the fade-in/out ramps in preview because:
    //   1. A fresh blur has start ≈ currentTime, so the linear ramp puts
    //      opacity at 0 → drawAnnotation would early-return → user sees
    //      nothing right after creating the blur.
    //   2. A partially-transparent blur copy mixed over the unblurred
    //      WebGL canvas reads as flicker, not a smooth fade — Canvas2D's
    //      globalAlpha applies to drawImage too.
    // Plus: while a blur is the *selected* annotation, always render it
    // even outside its [start, end] window. The user is actively editing
    // it; floating-point drift between `a.start` (captured at creation
    // time from `store.currentTime`) and `t` (from `videoEl.currentTime`
    // on the next animation frame) was making fresh blurs flicker on the
    // first few frames after placement. The export pipeline still honours
    // `start`/`end` exactly.
    const isBlur = a.kind.kind === "blur";
    const isSelected = a.id === store.selectedAnnotationId;
    if (isBlur) {
      if (!isSelected && (t < a.start || t > a.end)) return;
    } else if (opacity <= 0) {
      return;
    }
    if (!isCanvasDrawn(a.kind)) return; // text is rendered by TextAnnotationLayer

    if (a.kind.kind === "arrow") {
      drawArrow(ctx, a, opacity, t);
      return;
    }

    const r = rectPx();
    const box = normaliseBox(a.kind);
    const topLeft = projectUV(box.x, box.y, t);
    const bottomRight = projectUV(box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;
    if (w <= 0 || h <= 0) return;

    ctx.save();
    // Blur uses full preview opacity; other kinds honour the fade-ramp value.
    ctx.globalAlpha = isBlur ? 1 : opacity;
    applyGlow(ctx, a);

    ctx.beginPath();
    if (a.kind.kind === "rect") {
      const radius = Math.max(0, a.kind.radius * Math.min(r.w, r.h));
      if (radius > 0) {
        roundRectPath(ctx, x, y, w, h, radius);
      } else {
        ctx.rect(x, y, w, h);
      }
    } else if (a.kind.kind === "ellipse") {
      ctx.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
    } else if (a.kind.kind === "image") {
      ctx.rect(x, y, w, h);
    } else if (a.kind.kind === "blur") {
      // Real blur preview: copy the WebGL composite (full background +
      // padding + video) into the overlay canvas, blurred with the 2D
      // context's native `filter`. This is reliable across WebView
      // backends, unlike `backdrop-filter` against a GPU-promoted canvas.
      // Strength 0..1 maps to 0..32 px to match the export-side cap.
      const k = a.kind;
      if (compositeCanvasEl && w > 1 && h > 1) {
        const blurPx = Math.max(0.001, Math.min(32, k.strength * 32));
        // Source rect: same UV → canvas-px mapping, but in the WebGL
        // canvas's own backing-store coordinates. Both canvases share the
        // same DPR + size factor here because they both stretch to the
        // same `targetEl`, so we can read `compositeCanvasEl.width/height`
        // and treat its pixel space as proportional to ours.
        const srcW = compositeCanvasEl.width;
        const srcH = compositeCanvasEl.height;
        const dstW = canvasEl?.width ?? 0;
        const dstH = canvasEl?.height ?? 0;
        if (srcW > 0 && srcH > 0 && dstW > 0 && dstH > 0) {
          const sx = (x / dstW) * srcW;
          const sy = (y / dstH) * srcH;
          const sw = (w / dstW) * srcW;
          const sh = (h / dstH) * srcH;
          const radius = Math.max(0, k.radius * Math.min(r.w, r.h));
          ctx.save();
          ctx.beginPath();
          if (radius > 0) {
            roundRectPath(ctx, x, y, w, h, radius);
          } else {
            ctx.rect(x, y, w, h);
          }
          ctx.clip();
          // Setting `filter` on the 2D context applies to subsequent draws
          // — including `drawImage` from another canvas. Browser
          // implementations promote this to a GPU shader, so the cost is
          // negligible per blur region.
          ctx.filter = `blur(${blurPx.toFixed(2)}px)`;
          try {
            ctx.drawImage(
              compositeCanvasEl,
              sx,
              sy,
              sw,
              sh,
              x,
              y,
              w,
              h,
            );
          } catch {
            // drawImage can fail mid-frame if the source canvas was
            // resized between layout and render. Bail silently — the
            // next animation frame will repaint correctly.
          }
          ctx.filter = "none";
          // Variant tint sits on top of the blurred copy so it reads
          // as a deliberate privacy treatment rather than just a smudge.
          let tint: string | null = null;
          if (k.variant === "white") tint = "rgba(255,255,255,0.30)";
          else if (k.variant === "black") tint = "rgba(0,0,0,0.30)";
          else if (k.variant === "color") {
            const m = /^#?([0-9a-fA-F]{6})$/.exec(k.tintColor.trim());
            if (m) {
              const v = parseInt(m[1], 16);
              tint = `rgba(${(v >> 16) & 0xff},${(v >> 8) & 0xff},${v & 0xff},0.30)`;
            }
          }
          if (tint) {
            ctx.fillStyle = tint;
            ctx.fillRect(x, y, w, h);
          }
          ctx.restore();
        }
      }
    }

    if (a.kind.kind !== "image" && a.kind.kind !== "blur" && a.fill && a.fill !== "transparent") {
      ctx.fillStyle = a.fill;
      ctx.fill();
    }
    if (a.stroke.color && a.stroke.color !== "transparent" && a.stroke.width > 0) {
      const strokePx = Math.max(1, a.stroke.width * r.w);
      applyStrokeStyle(ctx, a, strokePx);
      ctx.strokeStyle = a.stroke.color;
      ctx.stroke();
    }

    ctx.restore();
  }

  function drawArrow(
    ctx: CanvasRenderingContext2D,
    a: Annotation,
    opacity: number,
    t: number,
  ) {
    if (a.kind.kind !== "arrow") return;
    const k = a.kind;
    const r = rectPx();
    const p1 = projectUV(k.x1, k.y1, t);
    const p2 = projectUV(k.x2, k.y2, t);
    const dx = p2.x - p1.x;
    const dy = p2.y - p1.y;
    const len = Math.hypot(dx, dy);
    if (len < 1) return;

    const strokePx = Math.max(2, a.stroke.width * r.w);
    const headLen = Math.max(strokePx * 2, k.headSize * len);
    const headWidth = headLen * 0.7;
    const ux = dx / len;
    const uy = dy / len;
    const lineEndX = p2.x - ux * headLen;
    const lineEndY = p2.y - uy * headLen;
    const nx = -uy;
    const ny = ux;

    ctx.save();
    ctx.globalAlpha = opacity;
    applyGlow(ctx, a);
    ctx.strokeStyle = a.stroke.color;
    ctx.fillStyle = a.stroke.color;
    applyStrokeStyle(ctx, a, strokePx);
    ctx.lineCap = "round";

    ctx.beginPath();
    ctx.moveTo(p1.x, p1.y);
    ctx.lineTo(lineEndX, lineEndY);
    ctx.stroke();

    // Reset dash before the head fill so it isn't striped.
    ctx.setLineDash([]);

    ctx.beginPath();
    ctx.moveTo(p2.x, p2.y);
    ctx.lineTo(lineEndX + nx * headWidth * 0.5, lineEndY + ny * headWidth * 0.5);
    ctx.lineTo(lineEndX - nx * headWidth * 0.5, lineEndY - ny * headWidth * 0.5);
    ctx.closePath();
    ctx.fill();

    ctx.restore();
  }

  /** Map AnnotationStroke.style → canvas dash pattern. */
  function applyStrokeStyle(
    ctx: CanvasRenderingContext2D,
    a: Annotation,
    strokePx: number,
  ) {
    ctx.lineWidth = strokePx;
    const style = a.stroke.style ?? "solid";
    if (style === "dashed") {
      ctx.setLineDash([8 * strokePx, 6 * strokePx]);
    } else if (style === "dotted") {
      ctx.setLineDash([2 * strokePx, 4 * strokePx]);
      ctx.lineCap = "round";
    } else {
      ctx.setLineDash([]);
    }
  }

  /** Apply the optional preview-only glow (rendered before fill/stroke). */
  function applyGlow(ctx: CanvasRenderingContext2D, a: Annotation) {
    if (!a.glow) return;
    const r = rectPx();
    ctx.shadowColor = a.glow.color;
    ctx.shadowBlur = Math.max(0, a.glow.blur * r.w);
    // Canvas shadow respects globalAlpha; pre-multiply to avoid double-darkening.
    ctx.globalAlpha = ctx.globalAlpha * Math.max(0, Math.min(1, a.glow.opacity));
  }

  function roundRectPath(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    w: number,
    h: number,
    r: number,
  ) {
    const maxR = Math.min(Math.abs(w) / 2, Math.abs(h) / 2);
    const rr = Math.min(r, maxR);
    ctx.moveTo(x + rr, y);
    ctx.lineTo(x + w - rr, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + rr);
    ctx.lineTo(x + w, y + h - rr);
    ctx.quadraticCurveTo(x + w, y + h, x + w - rr, y + h);
    ctx.lineTo(x + rr, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - rr);
    ctx.lineTo(x, y + rr);
    ctx.quadraticCurveTo(x, y, x + rr, y);
    ctx.closePath();
  }

  function drawSelection(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
    const dpr = getDpr();
    ctx.save();
    ctx.setLineDash([]);

    if (a.kind.kind === "arrow") {
      const p1 = projectUV(a.kind.x1, a.kind.y1, t);
      const p2 = projectUV(a.kind.x2, a.kind.y2, t);
      const hs = HANDLE_RADIUS_PX * dpr;
      for (const pt of [p1, p2]) {
        ctx.fillStyle = "#ffffff";
        ctx.fillRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
        ctx.strokeStyle = SELECTION_COLOUR;
        ctx.lineWidth = 1.5 * dpr;
        ctx.strokeRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
      }
      ctx.restore();
      return;
    }

    const box = normaliseBox(a.kind);
    const topLeft = projectUV(box.x, box.y, t);
    const bottomRight = projectUV(box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;

    ctx.strokeStyle = SELECTION_COLOUR;
    ctx.lineWidth = 1.5 * dpr;
    ctx.setLineDash([4 * dpr, 3 * dpr]);
    ctx.strokeRect(x, y, w, h);
    ctx.setLineDash([]);

    const hs = HANDLE_RADIUS_PX * dpr;
    const handles = handlePositions(x, y, w, h);
    for (const [, pt] of Object.entries(handles)) {
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
      ctx.strokeStyle = SELECTION_COLOUR;
      ctx.lineWidth = 1.5 * dpr;
      ctx.strokeRect(pt.x - hs, pt.y - hs, hs * 2, hs * 2);
    }
    ctx.restore();
  }

  /** Hover-flash from the layer panel: pulse a 2px outline around the shape. */
  function drawHoverFlash(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
    const dpr = getDpr();
    ctx.save();
    ctx.strokeStyle = HOVER_FLASH_COLOUR;
    ctx.lineWidth = 2 * dpr;
    ctx.setLineDash([]);

    if (a.kind.kind === "arrow") {
      const p1 = projectUV(a.kind.x1, a.kind.y1, t);
      const p2 = projectUV(a.kind.x2, a.kind.y2, t);
      ctx.beginPath();
      ctx.moveTo(p1.x, p1.y);
      ctx.lineTo(p2.x, p2.y);
      ctx.stroke();
      ctx.restore();
      return;
    }

    const box = normaliseBox(a.kind);
    const tl = projectUV(box.x, box.y, t);
    const br = projectUV(box.x + box.w, box.y + box.h, t);
    const pad = 4 * dpr;
    ctx.strokeRect(
      tl.x - pad,
      tl.y - pad,
      br.x - tl.x + pad * 2,
      br.y - tl.y + pad * 2,
    );
    ctx.restore();
  }

  //  Frame loop

  function draw() {
    if (!canvasEl || !store.metadata) return;
    resizeToContainer();
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

    if (store.annotationsGloballyHidden) return;

    const t = playbackTime();
    // Iterate by z-order so stacking is deterministic.
    const ordered = store.annotationsByZ;
    for (const a of ordered) {
      if (a.hidden) continue;
      const opacity = evalOpacity(a, t);
      drawAnnotation(ctx, a, opacity, t);
    }

    // Selection adornment + hover-flash only show on the Annotations tab so
    // the editing handles don't clutter the preview while the user is on
    // other panels.
    if (store.activePanel === "annotations") {
      const hover =
        store.hoveredAnnotationId && store.hoveredAnnotationId !== store.selectedAnnotationId
          ? store.annotations.find((a) => a.id === store.hoveredAnnotationId)
          : null;
      if (hover && !hover.hidden) drawHoverFlash(ctx, hover, t);

      const sel = store.annotations.find((a) => a.id === store.selectedAnnotationId);
      if (sel && !sel.hidden) drawSelection(ctx, sel, t);

      if (snapGuides.length > 0) drawSnapGuides(ctx, t);
    }
  }

  /** Draw the snap guides emitted during the active drag. Two guides max in
   *  practice (one per axis); the cap in `applySnap` enforces a hard ceiling. */
  function drawSnapGuides(ctx: CanvasRenderingContext2D, t: number) {
    const dpr = getDpr();
    const r = rectPx();
    if (r.w <= 0 || r.h <= 0) return;

    ctx.save();
    ctx.strokeStyle = SNAP_GUIDE_COLOUR;
    ctx.lineWidth = 1 * dpr;
    ctx.setLineDash([4 * dpr, 3 * dpr]);

    for (const g of snapGuides) {
      if (g.axis === "x") {
        const top = uvToCanvas(g.value, 0, r, evalZoom(zoomRegions(), t));
        const bot = uvToCanvas(g.value, 1, r, evalZoom(zoomRegions(), t));
        ctx.beginPath();
        ctx.moveTo(top.x, top.y);
        ctx.lineTo(bot.x, bot.y);
        ctx.stroke();
      } else {
        const left = uvToCanvas(0, g.value, r, evalZoom(zoomRegions(), t));
        const right = uvToCanvas(1, g.value, r, evalZoom(zoomRegions(), t));
        ctx.beginPath();
        ctx.moveTo(left.x, left.y);
        ctx.lineTo(right.x, right.y);
        ctx.stroke();
      }
    }
    ctx.restore();
  }

  function tick() {
    draw();
    rafHandle = requestAnimationFrame(tick);
  }

  function resizeToContainer() {
    if (!canvasEl || !targetEl) return;
    const rect = targetEl.getBoundingClientRect();
    const dpr = getDpr();
    const w = Math.max(1, Math.floor(rect.width * dpr));
    const h = Math.max(1, Math.floor(rect.height * dpr));
    if (canvasEl.width !== w || canvasEl.height !== h) {
      canvasEl.width = w;
      canvasEl.height = h;
    }
  }

  //  Pointer interaction

  function pickAnnotation(pt: { x: number; y: number }, t: number) {
    const dpr = getDpr();
    return hitTestAnnotation(pt, store.annotationsByZ, {
      rect: rectPx(),
      zoomRegions: zoomRegions(),
      t,
      handleSlop: HANDLE_RADIUS_PX * dpr + 2 * dpr,
      lineSlop: 6 * dpr,
      annotationSlop: 8 * dpr,
    });
  }

  function pickHandle(pt: { x: number; y: number }, a: Annotation, t: number) {
    const dpr = getDpr();
    return hitTestHandle(pt, a, {
      rect: rectPx(),
      zoomRegions: zoomRegions(),
      t,
      handleSlop: HANDLE_RADIUS_PX * dpr + 2 * dpr,
      lineSlop: 6 * dpr,
      annotationSlop: 8 * dpr,
    });
  }

  function handlePointerDown(e: PointerEvent) {
    if (!canvasEl || !store.metadata) return;
    if (store.annotationsGloballyHidden) return;
    const pt = pointerToCanvasPx(e);
    const t = playbackTime();

    // Selected annotation's handles come first (so you can resize over top of others).
    const selected = store.annotations.find((a) => a.id === store.selectedAnnotationId);
    if (selected && !selected.locked && !selected.hidden) {
      const hit = pickHandle(pt, selected, t);
      if (hit && hit !== "body") {
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        const box = normaliseBox(selected.kind);
        drag = { kind: "resize", id: selected.id, handle: hit, startBox: box };
        store.pushUndoState();
        e.preventDefault();
        return;
      }
      if (hit === "body") {
        // Body of the already-selected annotation → start moving immediately.
        // We deliberately skip the pickAnnotation path here so the annotation
        // can be moved during fade-in / fade-out windows where evalOpacity
        // would otherwise filter it out of the hit-test.
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        const pointerUV = unprojectUV(pt.x, pt.y, t);
        if (selected.kind.kind === "arrow") {
          drag = {
            kind: "move",
            id: selected.id,
            startX: selected.kind.x1,
            startY: selected.kind.y1,
            startX2: selected.kind.x2,
            startY2: selected.kind.y2,
            pointerStartUV: pointerUV,
          };
        } else {
          const box = normaliseBox(selected.kind);
          drag = {
            kind: "move",
            id: selected.id,
            startX: box.x,
            startY: box.y,
            pointerStartUV: pointerUV,
          };
        }
        store.pushUndoState();
        e.preventDefault();
        return;
      }
    }

    // Any annotation under the pointer → select and enter move mode.
    const hitAnno = pickAnnotation(pt, t);
    if (hitAnno) {
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      store.selectedAnnotationId = hitAnno.id;
      // Distance-from-segment uses pointToSegmentDist for arrows; reused so
      // future tools can hit-test against polylines without divergence.
      void pointToSegmentDist;
      const pointerUV = unprojectUV(pt.x, pt.y, t);
      if (hitAnno.kind.kind === "arrow") {
        drag = {
          kind: "move",
          id: hitAnno.id,
          startX: hitAnno.kind.x1,
          startY: hitAnno.kind.y1,
          startX2: hitAnno.kind.x2,
          startY2: hitAnno.kind.y2,
          pointerStartUV: pointerUV,
        };
      } else {
        const box = normaliseBox(hitAnno.kind);
        drag = {
          kind: "move",
          id: hitAnno.id,
          startX: box.x,
          startY: box.y,
          pointerStartUV: pointerUV,
        };
      }
      store.pushUndoState();
      e.preventDefault();
      return;
    }

    // No hit — if a tool is active, start placing a new annotation.
    const tool = store.annotationTool;
    if (tool) {
      const anchor = unprojectUV(pt.x, pt.y, t);
      let kind: AnnotationKind;
      switch (tool) {
        case "rect":
          kind = { kind: "rect", x: anchor.x, y: anchor.y, w: 0, h: 0, radius: 0.005 };
          break;
        case "ellipse":
          kind = { kind: "ellipse", x: anchor.x, y: anchor.y, w: 0, h: 0 };
          break;
        case "arrow":
          kind = {
            kind: "arrow",
            x1: anchor.x,
            y1: anchor.y,
            x2: anchor.x,
            y2: anchor.y,
            headSize: 0.15,
          };
          break;
        case "text":
          kind = {
            kind: "text",
            x: anchor.x,
            y: anchor.y,
            w: 0,
            h: 0,
            content: "Type here",
            fontFamily: "'Geist Variable', system-ui, sans-serif",
            fontSize: 0.06,
            fontWeight: 600,
            color: "#ffffff",
            align: "left",
            lineHeight: 1.2,
          };
          break;
        case "blur":
          kind = {
            kind: "blur",
            x: anchor.x,
            y: anchor.y,
            w: 0,
            h: 0,
            strength: 0.5,
            variant: "glass",
            tintColor: "#000000",
            radius: 0.005,
          };
          break;
        case "image":
          return;
        default:
          return;
      }
      const placed = store.addAnnotation(kind);
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      drag = { kind: "place", id: placed.id, anchor };
      e.preventDefault();
      return;
    }

    // Otherwise: deselect.
    store.selectedAnnotationId = null;
  }

  /** Build snap anchors from frame edges + every other annotation's box. */
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
      const box = normaliseBox(a.kind);
      anchors.push({ axis: "x", value: box.x });
      anchors.push({ axis: "x", value: box.x + box.w / 2 });
      anchors.push({ axis: "x", value: box.x + box.w });
      anchors.push({ axis: "y", value: box.y });
      anchors.push({ axis: "y", value: box.y + box.h / 2 });
      anchors.push({ axis: "y", value: box.y + box.h });
    }
    return anchors;
  }

  function applySnap(
    ux: number,
    uy: number,
    dragId: string | null,
    altHeld: boolean,
  ): { x: number; y: number } {
    if (altHeld || !store.annotationSnapEnabled) {
      snapGuides = [];
      return { x: ux, y: uy };
    }
    const anchors = buildSnapAnchors(dragId);
    const result = snap(ux, uy, anchors, 0.005, true);
    // Cap to 4 simultaneous guides (one per axis is the typical case; never
    // more than 2 from this fn, but keep the cap for safety).
    snapGuides = result.guides.slice(0, 4);
    return { x: result.x, y: result.y };
  }

  /** Refresh the hover state used for cursor affordance — runs only when no
   *  drag is in flight so the cursor flips between grab/resize as the user
   *  passes over annotations. */
  function refreshHover(pt: { x: number; y: number }, t: number) {
    if (drag) return;
    if (store.annotationTool) {
      hoverHandle = "tool";
      return;
    }
    const selected = store.annotations.find((a) => a.id === store.selectedAnnotationId);
    if (selected && !selected.locked && !selected.hidden) {
      const handle = pickHandle(pt, selected, t);
      if (handle && handle !== "body") {
        hoverHandle = handle;
        return;
      }
    }
    const hit = pickAnnotation(pt, t);
    hoverHandle = hit ? "body" : null;
  }

  function handlePointerMove(e: PointerEvent) {
    if (!drag) {
      refreshHover(pointerToCanvasPx(e), playbackTime());
      return;
    }
    const pt = pointerToCanvasPx(e);
    const t = playbackTime();
    const rawUv = unprojectUV(pt.x, pt.y, t);
    // Alt held bypasses snap, matching Figma. Snap is per-axis so an annotation
    // can lock to a horizontal guide while still tracking the cursor vertically.
    const uv = applySnap(rawUv.x, rawUv.y, drag.id, e.altKey);

    if (drag.kind === "place") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      if (anno.kind.kind === "arrow") {
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x2: uv.x, y2: uv.y },
        });
      } else if (
        anno.kind.kind === "rect" ||
        anno.kind.kind === "ellipse" ||
        anno.kind.kind === "text" ||
        anno.kind.kind === "image" ||
        anno.kind.kind === "blur"
      ) {
        const w = uv.x - drag.anchor.x;
        const h = uv.y - drag.anchor.y;
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: drag.anchor.x, y: drag.anchor.y, w, h },
        });
      }
    } else if (drag.kind === "move") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      const dx = uv.x - drag.pointerStartUV.x;
      const dy = uv.y - drag.pointerStartUV.y;
      if (anno.kind.kind === "arrow") {
        const sx2 = drag.startX2 ?? anno.kind.x2;
        const sy2 = drag.startY2 ?? anno.kind.y2;
        store.updateAnnotation(drag.id, {
          kind: {
            ...anno.kind,
            x1: drag.startX + dx,
            y1: drag.startY + dy,
            x2: sx2 + dx,
            y2: sy2 + dy,
          },
        });
      } else if (
        anno.kind.kind === "rect" ||
        anno.kind.kind === "ellipse" ||
        anno.kind.kind === "text" ||
        anno.kind.kind === "image" ||
        anno.kind.kind === "blur"
      ) {
        const newX = drag.startX + dx;
        const newY = drag.startY + dy;
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: newX, y: newY },
        });
      }
    } else if (drag.kind === "resize") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      if (anno.kind.kind === "arrow") {
        if (drag.handle === "p1") {
          store.updateAnnotation(drag.id, {
            kind: { ...anno.kind, x1: uv.x, y1: uv.y },
          });
        } else if (drag.handle === "p2") {
          store.updateAnnotation(drag.id, {
            kind: { ...anno.kind, x2: uv.x, y2: uv.y },
          });
        }
        return;
      }

      const b = drag.startBox;
      let nx = b.x;
      let ny = b.y;
      let nw = b.w;
      let nh = b.h;
      const h = drag.handle;
      if (h === "nw" || h === "w" || h === "sw") {
        nw = b.w + (b.x - uv.x);
        nx = uv.x;
      }
      if (h === "ne" || h === "e" || h === "se") {
        nw = uv.x - b.x;
      }
      if (h === "nw" || h === "n" || h === "ne") {
        nh = b.h + (b.y - uv.y);
        ny = uv.y;
      }
      if (h === "sw" || h === "s" || h === "se") {
        nh = uv.y - b.y;
      }
      if (
        anno.kind.kind === "rect" ||
        anno.kind.kind === "ellipse" ||
        anno.kind.kind === "text" ||
        anno.kind.kind === "image" ||
        anno.kind.kind === "blur"
      ) {
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: nx, y: ny, w: nw, h: nh },
        });
      }
    }
  }

  function handlePointerUp(e: PointerEvent) {
    if (!drag) return;
    (e.currentTarget as Element).releasePointerCapture(e.pointerId);
    // Drop snap guides immediately on release so the preview returns to
    // a clean state on click (no lingering guides between drags).
    snapGuides = [];
    if (drag.kind === "place") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (anno) {
        if (
          anno.kind.kind === "rect" ||
          anno.kind.kind === "ellipse" ||
          anno.kind.kind === "image" ||
          anno.kind.kind === "blur"
        ) {
          if (Math.abs(anno.kind.w) < 0.01 || Math.abs(anno.kind.h) < 0.01) {
            store.removeAnnotation(drag.id);
          }
        } else if (anno.kind.kind === "text") {
          if (Math.abs(anno.kind.w) < 0.04) {
            store.updateAnnotation(drag.id, {
              kind: { ...anno.kind, w: 0.25 },
            });
          }
          if (Math.abs(anno.kind.h) < 0.04) {
            store.updateAnnotation(drag.id, {
              kind: { ...anno.kind, h: anno.kind.fontSize * 1.6 },
            });
          }
        } else if (anno.kind.kind === "arrow") {
          const dx = anno.kind.x2 - anno.kind.x1;
          const dy = anno.kind.y2 - anno.kind.y1;
          if (Math.hypot(dx, dy) < 0.01) {
            store.removeAnnotation(drag.id);
          }
        }
      }
      // After placement, drop the tool so the user doesn't create stacked
      // shapes on their next click — matches Figma/Keynote behaviour.
      store.annotationTool = null;
    } else if (drag.kind === "resize" || drag.kind === "move") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (
        anno &&
        (anno.kind.kind === "rect" ||
          anno.kind.kind === "ellipse" ||
          anno.kind.kind === "text" ||
          anno.kind.kind === "image" ||
          anno.kind.kind === "blur")
      ) {
        const box = normaliseBox(anno.kind);
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: box.x, y: box.y, w: box.w, h: box.h },
        });
      }
    }
    drag = null;
  }

  function nudgeBy(dxUV: number, dyUV: number) {
    const id = store.selectedAnnotationId;
    if (!id) return;
    const a = store.annotations.find((x) => x.id === id);
    if (!a || a.locked || a.hidden) return;
    if (a.kind.kind === "arrow") {
      store.updateAnnotation(id, {
        kind: {
          ...a.kind,
          x1: a.kind.x1 + dxUV,
          y1: a.kind.y1 + dyUV,
          x2: a.kind.x2 + dxUV,
          y2: a.kind.y2 + dyUV,
        },
      });
    } else if (
      a.kind.kind === "rect" ||
      a.kind.kind === "ellipse" ||
      a.kind.kind === "text" ||
      a.kind.kind === "image"
    ) {
      store.updateAnnotation(id, {
        kind: { ...a.kind, x: a.kind.x + dxUV, y: a.kind.y + dyUV },
      });
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (store.annotationTool) {
        store.annotationTool = null;
        e.preventDefault();
      } else if (store.selectedAnnotationId) {
        store.selectedAnnotationId = null;
        e.preventDefault();
      }
      return;
    }
    if ((e.key === "Delete" || e.key === "Backspace") && store.selectedAnnotationId) {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      e.preventDefault();
      store.removeAnnotation(store.selectedAnnotationId);
      return;
    }

    // Z-order shortcuts and duplicate, gated to annotations tab + selection
    // so they don't fight other editor surfaces.
    if (
      store.activePanel === "annotations" &&
      store.selectedAnnotationId &&
      (e.metaKey || e.ctrlKey) &&
      !e.altKey
    ) {
      const target = e.target as HTMLElement | null;
      const inEditable =
        target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if (inEditable) return;
      if (e.key === "]") {
        e.preventDefault();
        store.reorderAnnotation(store.selectedAnnotationId, 1);
        return;
      }
      if (e.key === "[") {
        e.preventDefault();
        store.reorderAnnotation(store.selectedAnnotationId, -1);
        return;
      }
      if (e.key.toLowerCase() === "d" && !e.shiftKey) {
        e.preventDefault();
        store.duplicateAnnotation(store.selectedAnnotationId);
        return;
      }
    }

    // Arrow-key nudge — only when annotations tab is active and a non-locked
    // annotation is selected. Step is 1 device-px / 10 device-px in UV.
    if (
      store.activePanel === "annotations" &&
      store.selectedAnnotationId &&
      !e.metaKey &&
      !e.ctrlKey &&
      !e.altKey &&
      ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)
    ) {
      const target = e.target as HTMLElement | null;
      const inEditable =
        target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if (inEditable) return;
      const r = rectPx();
      if (r.w <= 0 || r.h <= 0) return;
      const stepX = (e.shiftKey ? 10 : 1) / Math.max(1, r.w);
      const stepY = (e.shiftKey ? 10 : 1) / Math.max(1, r.h);
      let dx = 0;
      let dy = 0;
      if (e.key === "ArrowLeft") dx = -stepX;
      if (e.key === "ArrowRight") dx = stepX;
      if (e.key === "ArrowUp") dy = -stepY;
      if (e.key === "ArrowDown") dy = stepY;
      e.preventDefault();
      nudgeBy(dx, dy);
    }
  }

  //  Lifecycle

  onMount(() => {
    tick();
    if (targetEl) {
      resizeObserver = new ResizeObserver(() => draw());
      resizeObserver.observe(targetEl);
    }
    window.addEventListener("keydown", handleKeyDown);
  });

  onDestroy(() => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle);
    resizeObserver?.disconnect();
    window.removeEventListener("keydown", handleKeyDown);
  });

  // Map a handle name to a CSS resize cursor so dragging from a corner shows
  // the diagonal arrow, edge handles show the axis arrow, and so on. Body
  // hovers show "grab" / "grabbing".
  function cursorForHandle(h: HandleName | "tool" | null): string {
    if (h === "tool") return "crosshair";
    switch (h) {
      case "nw":
      case "se":
        return "nwse-resize";
      case "ne":
      case "sw":
        return "nesw-resize";
      case "n":
      case "s":
        return "ns-resize";
      case "e":
      case "w":
        return "ew-resize";
      case "p1":
      case "p2":
        return "crosshair";
      case "body":
        return "grab";
      default:
        return "default";
    }
  }

  const canvasCursor = $derived.by(() => {
    if (store.annotationTool) return "crosshair";
    if (drag?.kind === "move") return "grabbing";
    if (drag?.kind === "resize") return cursorForHandle(drag.handle);
    return cursorForHandle(hoverHandle);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<canvas
  bind:this={canvasEl}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  onpointerleave={() => (hoverHandle = null)}
  class="absolute inset-0 h-full w-full"
  style:pointer-events={store.annotationsGloballyHidden ? "none" : "auto"}
  style:touch-action="none"
  style:cursor={canvasCursor}
></canvas>
