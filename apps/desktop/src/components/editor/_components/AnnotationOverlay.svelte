<script lang="ts">
  import { bezierY } from "$lib/easing/cubic-bezier";
  import type {
    Annotation,
    AnnotationKind,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { framePaddingPixels } from "$lib/stores/editor-store.svelte";
  import { onDestroy, onMount } from "svelte";

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    /** The container that wraps the WebGL preview canvas — we stretch to fit. */
    targetEl: HTMLElement | null;
  }

  let { store, videoEl, targetEl }: Props = $props();

  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rafHandle: number | null = null;
  let resizeObserver: ResizeObserver | null = null;

  //  Drag / placement state
  type HandleName =
    | "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "body"
    | "p1" | "p2"; // arrow endpoints
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

  const HANDLE_RADIUS_PX = 6; // CSS px half-size of resize handles
  const SELECTION_COLOUR = "#3b82f6";

  //  Helpers 

  function getDpr(): number {
    return window.devicePixelRatio || 1;
  }

  function compW(): number {
    const meta = store.metadata;
    if (!meta) return 0;
    const paddingPx = framePaddingPixels(store.padding, meta);
    return meta.width + paddingPx * 2;
  }

  /** Canvas device-px rect of the video region (mirror of the shader). */
  function videoRectPx(): { x: number; y: number; w: number; h: number } {
    if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
    const cw = canvasEl.width;
    const ch = canvasEl.height;
    const total = compW();
    const meta = store.metadata;
    const sourcePaddingPx = meta ? framePaddingPixels(store.padding, meta) : 0;
    const padPx = total > 0 ? (sourcePaddingPx / total) * cw : 0;
    return { x: padPx, y: padPx, w: cw - 2 * padPx, h: ch - 2 * padPx };
  }

  /** Current zoom scale + centre at playback time (mirror of VideoPreview). */
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

  /** Annotation opacity at time t via split-ramp (matches Focus semantics). */
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

  /** Annotation UV → canvas device-px, applying the shader's zoom transform. */
  function uvToCanvas(ux: number, uy: number, t: number): { x: number; y: number } {
    const rect = videoRectPx();
    const zoom = evalZoom(t);
    const preX = (ux - zoom.cx) * zoom.scale + zoom.cx;
    const preY = (uy - zoom.cy) * zoom.scale + zoom.cy;
    return {
      x: rect.x + preX * rect.w,
      y: rect.y + preY * rect.h,
    };
  }

  /** Canvas device-px → annotation UV (inverse of uvToCanvas). */
  function canvasToUV(cx: number, cy: number, t: number): { x: number; y: number } {
    const rect = videoRectPx();
    if (rect.w <= 0 || rect.h <= 0) return { x: 0, y: 0 };
    const zoom = evalZoom(t);
    const preX = (cx - rect.x) / rect.w;
    const preY = (cy - rect.y) / rect.h;
    return {
      x: (preX - zoom.cx) / zoom.scale + zoom.cx,
      y: (preY - zoom.cy) / zoom.scale + zoom.cy,
    };
  }

  /** Normalise bbox so width/height are positive (the user may drag "up-left"). */
  function normaliseBox(k: AnnotationKind): { x: number; y: number; w: number; h: number } {
    if (k.kind === "rect" || k.kind === "ellipse" || k.kind === "image") {
      const x = Math.min(k.x, k.x + k.w);
      const y = Math.min(k.y, k.y + k.h);
      return { x, y, w: Math.abs(k.w), h: Math.abs(k.h) };
    }
    if (k.kind === "arrow") {
      const x = Math.min(k.x1, k.x2);
      const y = Math.min(k.y1, k.y2);
      return { x, y, w: Math.abs(k.x2 - k.x1), h: Math.abs(k.y2 - k.y1) };
    }
    if (k.kind === "text") {
      const x = Math.min(k.x, k.x + k.w);
      const y = Math.min(k.y, k.y + k.h);
      return { x, y, w: Math.abs(k.w), h: Math.abs(k.h) };
    }
    return { x: 0, y: 0, w: 0, h: 0 };
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
    if (opacity <= 0) return;
    if (!isCanvasDrawn(a.kind)) return; // text is rendered by TextAnnotationLayer

    if (a.kind.kind === "arrow") {
      drawArrow(ctx, a, opacity, t);
      return;
    }

    const box = normaliseBox(a.kind);
    const topLeft = uvToCanvas(box.x, box.y, t);
    const bottomRight = uvToCanvas(box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;
    if (w <= 0 || h <= 0) return;

    ctx.save();
    ctx.globalAlpha = opacity;

    ctx.beginPath();
    if (a.kind.kind === "rect") {
      const radius = Math.max(
        0,
        a.kind.radius * Math.min(videoRectPx().w, videoRectPx().h),
      );
      if (radius > 0) {
        roundRectPath(ctx, x, y, w, h, radius);
      } else {
        ctx.rect(x, y, w, h);
      }
    } else if (a.kind.kind === "ellipse") {
      ctx.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
    } else if (a.kind.kind === "image") {
      // Image annotations are previewed as a placeholder rect — actual image
      // loading lands when the user-facing Image tool ships in Phase D.
      ctx.rect(x, y, w, h);
    }

    if (a.kind.kind !== "image" && a.fill && a.fill !== "transparent") {
      ctx.fillStyle = a.fill;
      ctx.fill();
    }
    if (a.stroke.color && a.stroke.color !== "transparent" && a.stroke.width > 0) {
      const strokePx = Math.max(1, a.stroke.width * videoRectPx().w);
      ctx.lineWidth = strokePx;
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
    const p1 = uvToCanvas(k.x1, k.y1, t);
    const p2 = uvToCanvas(k.x2, k.y2, t);
    const dx = p2.x - p1.x;
    const dy = p2.y - p1.y;
    const len = Math.hypot(dx, dy);
    if (len < 1) return;

    const strokePx = Math.max(2, a.stroke.width * videoRectPx().w);
    const headLen = Math.max(strokePx * 2, k.headSize * len);
    const headWidth = headLen * 0.7;
    const ux = dx / len;
    const uy = dy / len;
    // Trim the line at the head's base so the capsule end doesn't poke
    // through the triangle.
    const lineEndX = p2.x - ux * headLen;
    const lineEndY = p2.y - uy * headLen;
    const nx = -uy;
    const ny = ux;

    ctx.save();
    ctx.globalAlpha = opacity;
    ctx.strokeStyle = a.stroke.color;
    ctx.fillStyle = a.stroke.color;
    ctx.lineWidth = strokePx;
    ctx.lineCap = "round";

    ctx.beginPath();
    ctx.moveTo(p1.x, p1.y);
    ctx.lineTo(lineEndX, lineEndY);
    ctx.stroke();

    ctx.beginPath();
    ctx.moveTo(p2.x, p2.y);
    ctx.lineTo(lineEndX + nx * headWidth * 0.5, lineEndY + ny * headWidth * 0.5);
    ctx.lineTo(lineEndX - nx * headWidth * 0.5, lineEndY - ny * headWidth * 0.5);
    ctx.closePath();
    ctx.fill();

    ctx.restore();
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

    if (a.kind.kind === "arrow") {
      // Two endpoint handles only. No bounding-box dashed border (the arrow
      // itself indicates selection bounds, and a box would be visually
      // misleading for a non-rect primitive).
      const p1 = uvToCanvas(a.kind.x1, a.kind.y1, t);
      const p2 = uvToCanvas(a.kind.x2, a.kind.y2, t);
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
    const topLeft = uvToCanvas(box.x, box.y, t);
    const bottomRight = uvToCanvas(box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;

    ctx.strokeStyle = SELECTION_COLOUR;
    ctx.lineWidth = 1.5 * dpr;
    ctx.setLineDash([4 * dpr, 3 * dpr]);
    ctx.strokeRect(x, y, w, h);
    ctx.setLineDash([]);

    // 8 handles for box-shaped annotations.
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

  function handlePositions(
    x: number,
    y: number,
    w: number,
    h: number,
  ): Record<
    "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w",
    { x: number; y: number }
  > {
    return {
      nw: { x, y },
      n: { x: x + w / 2, y },
      ne: { x: x + w, y },
      e: { x: x + w, y: y + h / 2 },
      se: { x: x + w, y: y + h },
      s: { x: x + w / 2, y: y + h },
      sw: { x, y: y + h },
      w: { x, y: y + h / 2 },
    };
  }

  //  Frame loop 

  function draw() {
    if (!canvasEl || !store.metadata) return;
    resizeToContainer();
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

    const t = playbackTime();
    for (const a of store.annotations) {
      const opacity = evalOpacity(a, t);
      drawAnnotation(ctx, a, opacity, t);
    }

    // Selection adornment only shows on the Annotations tab so the editing
    // handles don't clutter the preview while the user is on other panels.
    if (store.activePanel === "annotations") {
      const sel = store.annotations.find((a) => a.id === store.selectedAnnotationId);
      if (sel) drawSelection(ctx, sel, t);
    }
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

  function hitTestHandle(
    pt: { x: number; y: number },
    a: Annotation,
    t: number,
  ): HandleName | null {
    const dpr = getDpr();
    const slop = HANDLE_RADIUS_PX * dpr + 2 * dpr;

    if (a.kind.kind === "arrow") {
      const p1 = uvToCanvas(a.kind.x1, a.kind.y1, t);
      const p2 = uvToCanvas(a.kind.x2, a.kind.y2, t);
      if (Math.abs(pt.x - p1.x) <= slop && Math.abs(pt.y - p1.y) <= slop) {
        return "p1";
      }
      if (Math.abs(pt.x - p2.x) <= slop && Math.abs(pt.y - p2.y) <= slop) {
        return "p2";
      }
      // Hit on the line itself = move (treat as "body").
      if (pointToSegmentDist(pt, p1, p2) <= 6 * dpr) return "body";
      return null;
    }

    const box = normaliseBox(a.kind);
    const topLeft = uvToCanvas(box.x, box.y, t);
    const bottomRight = uvToCanvas(box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;
    const handles = handlePositions(x, y, w, h);
    for (const [name, p] of Object.entries(handles)) {
      if (Math.abs(pt.x - p.x) <= slop && Math.abs(pt.y - p.y) <= slop) {
        return name as HandleName;
      }
    }
    // Body hit (for moving).
    if (pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h) return "body";
    return null;
  }

  /** Shortest pixel distance from point `p` to the line segment `a→b`. */
  function pointToSegmentDist(
    p: { x: number; y: number },
    a: { x: number; y: number },
    b: { x: number; y: number },
  ): number {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq === 0) return Math.hypot(p.x - a.x, p.y - a.y);
    const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq));
    const cx = a.x + t * dx;
    const cy = a.y + t * dy;
    return Math.hypot(p.x - cx, p.y - cy);
  }

  function hitTestAnnotation(pt: { x: number; y: number }, t: number): Annotation | null {
    const dpr = getDpr();
    // Iterate in reverse (topmost draw last → last-to-first on hit).
    for (let i = store.annotations.length - 1; i >= 0; i--) {
      const a = store.annotations[i];
      if (evalOpacity(a, t) <= 0.05) continue;
      // Text annotations are HTML elements; let pointer events fall through
      // to TextAnnotationLayer's own hit-test instead of grabbing them here.
      if (a.kind.kind === "text") continue;
      if (a.kind.kind === "arrow") {
        const p1 = uvToCanvas(a.kind.x1, a.kind.y1, t);
        const p2 = uvToCanvas(a.kind.x2, a.kind.y2, t);
        if (pointToSegmentDist(pt, p1, p2) <= 8 * dpr) return a;
        continue;
      }
      const box = normaliseBox(a.kind);
      const topLeft = uvToCanvas(box.x, box.y, t);
      const bottomRight = uvToCanvas(box.x + box.w, box.y + box.h, t);
      if (
        pt.x >= topLeft.x &&
        pt.x <= bottomRight.x &&
        pt.y >= topLeft.y &&
        pt.y <= bottomRight.y
      ) {
        return a;
      }
    }
    return null;
  }

  function handlePointerDown(e: PointerEvent) {
    if (!canvasEl || !store.metadata) return;
    const pt = pointerToCanvasPx(e);
    const t = playbackTime();

    // Selected annotation's handles come first (so you can resize over top of others).
    const selected = store.annotations.find((a) => a.id === store.selectedAnnotationId);
    if (selected) {
      const hit = hitTestHandle(pt, selected, t);
      if (hit && hit !== "body") {
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        const box = normaliseBox(selected.kind);
        drag = { kind: "resize", id: selected.id, handle: hit, startBox: box };
        store.pushUndoState();
        e.preventDefault();
        return;
      }
    }

    // Any annotation under the pointer → select and enter move mode.
    const hitAnno = hitTestAnnotation(pt, t);
    if (hitAnno) {
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      store.selectedAnnotationId = hitAnno.id;
      const pointerUV = canvasToUV(pt.x, pt.y, t);
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
      const anchor = canvasToUV(pt.x, pt.y, t);
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
        case "image":
          // Image tool is currently disabled in the palette — guard anyway.
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

  function handlePointerMove(e: PointerEvent) {
    if (!drag) return;
    const pt = pointerToCanvasPx(e);
    const t = playbackTime();
    const uv = canvasToUV(pt.x, pt.y, t);

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
        anno.kind.kind === "image"
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
        anno.kind.kind === "image"
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
      // Arrow resize = move one endpoint.
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
        anno.kind.kind === "image"
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
    if (drag.kind === "place") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (anno) {
        if (anno.kind.kind === "rect" || anno.kind.kind === "ellipse" || anno.kind.kind === "image") {
          if (Math.abs(anno.kind.w) < 0.01 || Math.abs(anno.kind.h) < 0.01) {
            store.removeAnnotation(drag.id);
          }
        } else if (anno.kind.kind === "text") {
          // For text, allow tiny boxes to expand to a sensible default so a
          // single click still creates a usable text annotation.
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
      // Re-normalise box-shaped kinds so stored coordinates always have
      // positive w/h. Arrows are stored as endpoint pairs and don't need it.
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (
        anno &&
        (anno.kind.kind === "rect" ||
          anno.kind.kind === "ellipse" ||
          anno.kind.kind === "text" ||
          anno.kind.kind === "image")
      ) {
        const box = normaliseBox(anno.kind);
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: box.x, y: box.y, w: box.w, h: box.h },
        });
      }
    }
    drag = null;
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
    }
    if ((e.key === "Delete" || e.key === "Backspace") && store.selectedAnnotationId) {
      // Don't fight text inputs.
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      e.preventDefault();
      store.removeAnnotation(store.selectedAnnotationId);
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

  // When the tool changes (or an annotation is selected), change the cursor.
  const canvasCursor = $derived.by(() => {
    if (store.annotationTool) return "crosshair";
    if (drag?.kind === "move") return "grabbing";
    if (drag?.kind === "resize") return "nwse-resize";
    return "default";
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- Annotations themselves always render (they're part of the composed
     preview), but pointer interaction is gated to the Annotations tab so the
     user can't accidentally drag handles while editing audio/cursor/focus. -->
<canvas
  bind:this={canvasEl}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerUp}
  class="absolute inset-0 h-full w-full"
  class:pointer-events-auto={store.activePanel === "annotations"}
  class:pointer-events-none={store.activePanel !== "annotations"}
  style:cursor={canvasCursor}
></canvas>
