<script lang="ts">
  import { evalOpacity, evalZoom } from "$lib/annotations/eval";
  import {
    handlePositions,
    hitTestAnnotation,
    hitTestHandle,
    pointToSegmentDist,
    type HandleName,
  } from "$lib/annotations/hit";
  import {
    canvasToUV,
    compositionRectPx,
    normaliseBox,
    uvToCanvas,
    videoRectPx,
    type Rect,
  } from "$lib/annotations/uv";
  import { snap, snapBox, type SnapAnchor } from "$lib/annotations/snap";
  import {
    constrain45,
    constrainSquare,
    isCornerHandle,
    lockAspect,
  } from "$lib/annotations/resize-constraints";
  import {
    disposeCanvasTokens,
    selectionPalette,
  } from "$lib/annotations/canvas-tokens";
  import {
    arrowGeometry,
    blurTint,
    cursorForHandle,
    HANDLE_CORNER_PX,
    HANDLE_RADIUS_PX,
    IDENTITY_ZOOM,
    roundRectPath,
    strokeDashPattern,
    withAlpha,
  } from "./annotation-draw.logic";
  import { buildAnnotationSnapAnchors } from "./annotation-snap.logic";
  import type {
    Annotation,
    AnnotationAnchor,
    AnnotationKind,
    EditorStore,
  } from "$lib/stores/editor-store.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
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
  // Container CSS size, cached from a ResizeObserver so the rAF loop doesn't
  // force a layout with getBoundingClientRect() every frame.
  let targetSize = { w: 0, h: 0 };

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
  // Undo is pushed on the first real move of a move/resize drag, not at
  // pointer-down, so a pure select-click leaves no no-op entry. Placement
  // pushes via addAnnotation, so it starts "already pushed".
  let dragUndoPushed = true;
  // Active snap guides for the current drag, in UV space. Cleared on
  // pointerup. Capped to 4 simultaneous guides to avoid visual noise.
  let snapGuides: SnapAnchor[] = $state([]);
  // What's under the pointer, used purely for cursor affordance ("grab" on
  // body, "nwse-resize" / "ns-resize" / etc on handles). Cleared on leave.
  let hoverHandle: HandleName | null | "tool" = $state(null);

  // Thin wrappers around shared geometry modules; this file owns rendering +
  // interaction state, not the math.
  function getDpr(): number {
    return window.devicePixelRatio || 1;
  }

  function videoRect(): Rect {
    if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
    return videoRectPx(
      canvasEl.width,
      canvasEl.height,
      store.metadata,
      store.padding,
      store.outputAspect,
    );
  }

  function compRect(): Rect {
    if (!canvasEl) return { x: 0, y: 0, w: 0, h: 0 };
    return compositionRectPx(
      canvasEl.width,
      canvasEl.height,
      store.metadata,
      store.padding,
      store.outputAspect,
    );
  }

  /** Rect an annotation projects onto: the padded frame when anchored to
   *  "frame", otherwise the video region (which the zoom transform then acts
   *  on). Accepts anything with an optional `anchor` so placement can pass a
   *  bare object. */
  function rectFor(a: { anchor?: AnnotationAnchor }): Rect {
    return a.anchor === "frame" ? compRect() : videoRect();
  }

  /** Frame-anchored annotations ignore zoom; video-anchored ones track it. */
  function zoomFor(a: { anchor?: AnnotationAnchor }, t: number) {
    return a.anchor === "frame" ? IDENTITY_ZOOM : evalZoom(store.zoomRegions, t);
  }

  function projectA(
    a: { anchor?: AnnotationAnchor },
    ux: number,
    uy: number,
    t: number,
  ) {
    return uvToCanvas(ux, uy, rectFor(a), zoomFor(a, t));
  }

  function unprojectA(
    a: { anchor?: AnnotationAnchor },
    cx: number,
    cy: number,
    t: number,
  ) {
    return canvasToUV(cx, cy, rectFor(a), zoomFor(a, t));
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
    // Blur bypasses the fade ramps in preview: a fresh blur (start ≈ currentTime)
    // would ramp from opacity 0 and early-return, and a half-transparent blur
    // copy over the unblurred canvas reads as flicker (globalAlpha applies to
    // drawImage). When a blur is selected, render it even outside [start, end] —
    // float drift between a.start and t flickered fresh blurs on placement.
    // Export still honours start/end exactly.
    const isBlur = a.kind.kind === "blur";
    const isSelected = a.id === store.selectedAnnotationId;
    const editing = store.activePanel === "annotations";
    // Outside its time window an annotation is invisible. Keep showing the
    // SELECTED one as a dim ghost while editing so moving/resizing it (its
    // handles draw regardless of time) doesn't make it vanish under the cursor.
    let renderOpacity = opacity;
    if (isBlur) {
      if (!isSelected && (t < a.start || t > a.end)) return;
    } else if (opacity <= 0) {
      if (isSelected && editing) renderOpacity = 0.35;
      else return;
    }
    if (!isCanvasDrawn(a.kind)) return; // text is rendered by TextAnnotationLayer

    if (a.kind.kind === "arrow") {
      drawArrow(ctx, a, renderOpacity, t);
      return;
    }

    const r = rectFor(a);
    const box = normaliseBox(a.kind);
    const topLeft = projectA(a, box.x, box.y, t);
    const bottomRight = projectA(a, box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;
    if (w <= 0 || h <= 0) return;

    ctx.save();
    // Blur uses full preview opacity; other kinds honour the fade-ramp value.
    ctx.globalAlpha = isBlur ? 1 : renderOpacity;
    applyGlow(ctx, a);

    ctx.beginPath();
    if (a.kind.kind === "rect") {
      // Radius = fraction (0..0.5) of the box's shorter side, so 100% rounds
      // fully regardless of box size (roundRectPath clamps to half).
      const radius = Math.max(0, a.kind.radius * Math.min(w, h));
      if (radius > 0) {
        roundRectPath(ctx, x, y, w, h, radius);
      } else {
        ctx.rect(x, y, w, h);
      }
    } else if (a.kind.kind === "ellipse") {
      ctx.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
    } else if (a.kind.kind === "image") {
      // Drawn directly (not via the shared fill/stroke path below) so we can
      // control the border and shadow independently of the shape kinds.
      drawImageAnnotation(ctx, a.kind, x, y, w, h);
      if (a.stroke.color && a.stroke.color !== "transparent" && a.stroke.width > 0) {
        const cornerPx = Math.max(0, a.kind.radius * Math.min(Math.abs(w), Math.abs(h)));
        const strokePx = Math.max(1, a.stroke.width * r.w);
        // Border sits on the image; the glow already fired on the image itself.
        ctx.shadowColor = "transparent";
        ctx.shadowBlur = 0;
        ctx.beginPath();
        if (cornerPx > 0.5) roundRectPath(ctx, x, y, w, h, cornerPx);
        else ctx.rect(x, y, w, h);
        applyStrokeStyle(ctx, a, strokePx);
        ctx.strokeStyle = a.stroke.color;
        ctx.stroke();
      }
    } else if (a.kind.kind === "blur") {
      // Copy the WebGL composite into the overlay canvas, blurred via the 2D
      // context's native `filter` — reliable across WebView backends, unlike
      // backdrop-filter on a GPU-promoted canvas.
      const k = a.kind;
      if (compositeCanvasEl && w > 1 && h > 1) {
        // Source rect in the WebGL canvas's backing-store coords. Both canvases
        // stretch to the same targetEl, so its pixel space is proportional to ours.
        const srcW = compositeCanvasEl.width;
        const srcH = compositeCanvasEl.height;
        const dstW = canvasEl?.width ?? 0;
        const dstH = canvasEl?.height ?? 0;
        // Match the export radius: strength × 12% of the frame's shorter side.
        // A single gaussian ~= the export's 3-pass box of the same radius, so
        // the editor frosting reads the same strength as the rendered file.
        const blurPx = Math.max(0.001, k.strength * 0.12 * Math.min(dstW, dstH));
        if (srcW > 0 && srcH > 0 && dstW > 0 && dstH > 0) {
          // Sample a margin of real surrounding pixels around the region so the
          // blur has content to pull from. Without it, a large radius samples
          // the transparent edge, its alpha washes out, and the sharp video
          // shows through — which reads as LESS blur past ~40% strength. (The
          // export box-blurs an edge-clamped crop, the same idea.)
          const m = Math.ceil(blurPx);
          const ex = x - m;
          const ey = y - m;
          const ew = w + 2 * m;
          const eh = h + 2 * m;
          const esx = (ex / dstW) * srcW;
          const esy = (ey / dstH) * srcH;
          const esw = (ew / dstW) * srcW;
          const esh = (eh / dstH) * srcH;
          // Corner radius as a fraction (0..0.5) of the region's shorter side.
          const radius = Math.max(0, k.radius * Math.min(w, h));
          const bw = Math.max(1, Math.round(w));
          const bh = Math.max(1, Math.round(h));
          const octx = getBlurScratch(bw, bh);
          if (octx) {
            octx.clearRect(0, 0, bw, bh);
            // Blur the composite into the scratch. The expanded source (esx..)
            // maps so the region lands at (0,0,w,h); the -m offset gives the
            // blur real margin pixels, and the scratch bounds clip the spill.
            octx.filter = `blur(${blurPx.toFixed(2)}px)`;
            try {
              octx.drawImage(compositeCanvasEl, esx, esy, esw, esh, -m, -m, ew, eh);
            } catch {
              // source not readable this frame; next rAF repaints.
            }
            octx.filter = "none";
            // Variant tint on top of the blurred copy so it reads as a
            // deliberate privacy treatment rather than just a smudge.
            const tint = blurTint(k.variant, k.tintColor, k.strength, a.opacity ?? 1);
            if (tint) {
              octx.fillStyle = tint;
              octx.fillRect(0, 0, bw, bh);
            }
            // Composite onto the overlay under the rounded clip (no filter in
            // effect, so the rounded corners are honoured).
            ctx.save();
            ctx.beginPath();
            if (radius > 0) {
              roundRectPath(ctx, x, y, w, h, radius);
            } else {
              ctx.rect(x, y, w, h);
            }
            ctx.clip();
            ctx.drawImage(blurScratch!, x, y, w, h);
            ctx.restore();
          }
        }
      }
    }

    if (a.kind.kind !== "image" && a.kind.kind !== "blur" && a.fill && a.fill !== "transparent") {
      ctx.fillStyle = a.fill;
      ctx.fill();
    }
    // Image draws its own border in its branch (above) so it can sit over the
    // image and skip the glow; other kinds stroke the path built above.
    if (
      a.kind.kind !== "image" &&
      a.stroke.color &&
      a.stroke.color !== "transparent" &&
      a.stroke.width > 0
    ) {
      const strokePx = Math.max(1, a.stroke.width * r.w);
      applyStrokeStyle(ctx, a, strokePx);
      ctx.strokeStyle = a.stroke.color;
      ctx.stroke();
    }

    ctx.restore();
  }

  // Decoded <img> per source path, reused across frames. The rAF loop repaints
  // continuously, so a load that finishes later shows up on the next frame.
  type ImageEntry = {
    img: HTMLImageElement;
    ready: boolean;
    failed: boolean;
    failedAt: number;
  };
  const imageCache = new Map<string, ImageEntry>();
  const IMAGE_RETRY_MS = 4000;

  function getImage(path: string): ImageEntry {
    let entry = imageCache.get(path);
    // Retry a failed load after a delay so a restored/renamed file recovers
    // within the session instead of showing the placeholder forever.
    if (entry?.failed && Date.now() - entry.failedAt > IMAGE_RETRY_MS) {
      imageCache.delete(path);
      entry = undefined;
    }
    if (!entry) {
      const img = new Image();
      entry = { img, ready: false, failed: false, failedAt: 0 };
      const e = entry;
      img.onload = () => {
        e.ready = true;
      };
      img.onerror = () => {
        e.failed = true;
        e.failedAt = Date.now();
      };
      img.src = convertFileSrc(path);
      imageCache.set(path, entry);
    }
    return entry;
  }

  // Evict cached bitmaps no longer referenced by any annotation, so replacing
  // or deleting images doesn't accumulate decoded images for the editor's life.
  $effect(() => {
    const live = new Set<string>();
    for (const a of store.annotations) {
      if (a.kind.kind === "image" && a.kind.path) live.add(a.kind.path);
    }
    const stale: string[] = [];
    for (const path of imageCache.keys()) {
      if (!live.has(path)) stale.push(path);
    }
    for (const path of stale) imageCache.delete(path);
  });

  // Offscreen scratch canvas for blur. We render the blur + tint here
  // (rectangular) then composite onto the overlay under a rounded clip — a
  // rounded `ctx.clip()` is NOT reliably honoured while `ctx.filter = blur()`
  // is active (WebView2/Chromium blurs to the full bounding box), so the
  // corners are applied afterwards with no filter in effect.
  let blurScratch: HTMLCanvasElement | null = null;
  function getBlurScratch(w: number, h: number): CanvasRenderingContext2D | null {
    if (!blurScratch) blurScratch = document.createElement("canvas");
    if (blurScratch.width !== w || blurScratch.height !== h) {
      blurScratch.width = w;
      blurScratch.height = h;
    }
    return blurScratch.getContext("2d");
  }

  function drawImageAnnotation(
    ctx: CanvasRenderingContext2D,
    k: Extract<AnnotationKind, { kind: "image" }>,
    x: number,
    y: number,
    w: number,
    h: number,
  ) {
    const entry = k.path ? getImage(k.path) : null;
    if (entry?.ready) {
      ctx.save();
      ctx.globalAlpha *= Math.max(0, Math.min(1, k.opacity));
      const cornerPx = Math.max(0, (k.radius ?? 0) * Math.min(Math.abs(w), Math.abs(h)));
      if (cornerPx > 0.5) {
        // A clip would swallow the glow shadow (applyGlow set it on ctx), so
        // cast it from the rounded outline first, then draw the image clipped
        // with the shadow disabled.
        if (ctx.shadowBlur > 0) {
          ctx.beginPath();
          roundRectPath(ctx, x, y, w, h, cornerPx);
          ctx.fill();
          ctx.shadowColor = "transparent";
          ctx.shadowBlur = 0;
        }
        ctx.beginPath();
        roundRectPath(ctx, x, y, w, h, cornerPx);
        ctx.clip();
      }
      try {
        ctx.drawImage(entry.img, x, y, w, h);
      } catch {
        // Source not decodable this frame (e.g. resized mid-render); the next
        // rAF repaints correctly.
      }
      ctx.restore();
      return;
    }
    // Placeholder while loading (or on error): a faint box so the region stays
    // visible and selectable before pixels arrive.
    ctx.save();
    ctx.fillStyle = "rgba(120, 120, 120, 0.12)";
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = "rgba(120, 120, 120, 0.5)";
    ctx.setLineDash([6 * getDpr(), 4 * getDpr()]);
    ctx.lineWidth = getDpr();
    ctx.strokeRect(x, y, w, h);
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
    const r = rectFor(a);
    const p1 = projectA(a, k.x1, k.y1, t);
    const p2 = projectA(a, k.x2, k.y2, t);
    const strokePx = Math.max(2, a.stroke.width * r.w);
    const geo = arrowGeometry(p1, p2, strokePx, k.headSize);
    if (!geo) return;

    ctx.save();
    ctx.globalAlpha = opacity;
    applyGlow(ctx, a);
    ctx.strokeStyle = a.stroke.color;
    ctx.fillStyle = a.stroke.color;
    applyStrokeStyle(ctx, a, strokePx);
    ctx.lineCap = "round";

    ctx.beginPath();
    ctx.moveTo(p1.x, p1.y);
    ctx.lineTo(geo.lineEnd.x, geo.lineEnd.y);
    ctx.stroke();

    // Reset dash before the head fill so it isn't striped.
    ctx.setLineDash([]);

    ctx.beginPath();
    ctx.moveTo(geo.tip.x, geo.tip.y);
    ctx.lineTo(geo.left.x, geo.left.y);
    ctx.lineTo(geo.right.x, geo.right.y);
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
    ctx.setLineDash(strokeDashPattern(style, strokePx));
    if (style === "dotted") ctx.lineCap = "round";
  }

  /** Apply the optional glow (rendered before fill/stroke; exported for all
   *  canvas kinds except arrow). */
  function applyGlow(ctx: CanvasRenderingContext2D, a: Annotation) {
    if (!a.glow) return;
    const r = rectFor(a);
    // Bake glow opacity into the shadow colour, NOT globalAlpha. The export dims
    // only the cast glow and keeps the shape at full opacity (cursor_export.rs
    // draw_shape_shadow/draw_image_shadow scale `alpha × glow.opacity`); folding
    // it into globalAlpha here would fade the whole shape and break parity.
    ctx.shadowColor = withAlpha(a.glow.color, a.glow.opacity);
    ctx.shadowBlur = Math.max(0, a.glow.blur * r.w);
  }

  /** A single resize grip: a rounded square with the surface fill, a crisp
   *  primary border and a soft drop shadow, matching the recording overlay's
   *  handle language. Shadow is applied to the fill only. */
  function drawHandle(
    ctx: CanvasRenderingContext2D,
    cx: number,
    cy: number,
    dpr: number,
    palette: ReturnType<typeof selectionPalette>,
  ) {
    const hs = HANDLE_RADIUS_PX * dpr;
    const r = HANDLE_CORNER_PX * dpr;
    ctx.beginPath();
    roundRectPath(ctx, cx - hs, cy - hs, hs * 2, hs * 2, r);
    ctx.save();
    ctx.shadowColor = "rgba(0, 0, 0, 0.25)";
    ctx.shadowBlur = 3 * dpr;
    ctx.shadowOffsetY = 0.5 * dpr;
    ctx.fillStyle = palette.surface;
    ctx.fill();
    ctx.restore();
    ctx.lineWidth = 1.5 * dpr;
    ctx.strokeStyle = palette.accent;
    ctx.stroke();
  }

  function drawSelection(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
    const dpr = getDpr();
    const palette = selectionPalette();
    ctx.save();
    ctx.setLineDash([]);

    if (a.kind.kind === "arrow") {
      const p1 = projectA(a, a.kind.x1, a.kind.y1, t);
      const p2 = projectA(a, a.kind.x2, a.kind.y2, t);
      for (const pt of [p1, p2]) drawHandle(ctx, pt.x, pt.y, dpr, palette);
      ctx.restore();
      return;
    }

    const box = normaliseBox(a.kind);
    const topLeft = projectA(a, box.x, box.y, t);
    const bottomRight = projectA(a, box.x + box.w, box.y + box.h, t);
    const x = topLeft.x;
    const y = topLeft.y;
    const w = bottomRight.x - topLeft.x;
    const h = bottomRight.y - topLeft.y;

    // Soft outer ring then the crisp primary border — mirrors the recording
    // area selection's `border-primary ring-primary/40`.
    ctx.strokeStyle = palette.accentRing;
    ctx.lineWidth = 3 * dpr;
    ctx.strokeRect(x, y, w, h);
    ctx.strokeStyle = palette.accent;
    ctx.lineWidth = 1.5 * dpr;
    ctx.strokeRect(x, y, w, h);

    const handles = handlePositions(x, y, w, h);
    for (const [, pt] of Object.entries(handles)) {
      drawHandle(ctx, pt.x, pt.y, dpr, palette);
    }
    ctx.restore();
  }

  /** Size badge pinned to the top-left of the box while placing or resizing,
   *  showing the annotation's dimensions in output-video pixels. Mirrors the
   *  recording overlay's `bg-primary` dimension chip. */
  function drawSizeBadge(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
    if (a.kind.kind === "arrow" || !store.metadata) return;
    const dpr = getDpr();
    const palette = selectionPalette();
    const box = normaliseBox(a.kind);
    const tl = projectA(a, box.x, box.y, t);
    const wPx = Math.round(box.w * store.metadata.width);
    const hPx = Math.round(box.h * store.metadata.height);
    const label = `${wPx} × ${hPx}`;

    ctx.save();
    ctx.font = `600 ${11 * dpr}px ${palette.monoFamily}`;
    ctx.textBaseline = "middle";
    const padX = 6 * dpr;
    const chipH = 18 * dpr;
    const textW = ctx.measureText(label).width;
    const chipW = textW + padX * 2;
    const chipX = tl.x;
    const chipY = Math.max(tl.y - chipH - 4 * dpr, 2 * dpr);

    ctx.beginPath();
    roundRectPath(ctx, chipX, chipY, chipW, chipH, 3 * dpr);
    ctx.fillStyle = palette.accent;
    ctx.fill();
    ctx.fillStyle = palette.onAccent;
    ctx.fillText(label, chipX + padX, chipY + chipH / 2 + 0.5 * dpr);
    ctx.restore();
  }

  /** Hover-flash from the layer panel: pulse a 2px outline around the shape. */
  function drawHoverFlash(ctx: CanvasRenderingContext2D, a: Annotation, t: number) {
    const dpr = getDpr();
    ctx.save();
    ctx.strokeStyle = selectionPalette().accentMuted;
    ctx.lineWidth = 2 * dpr;
    ctx.setLineDash([]);

    if (a.kind.kind === "arrow") {
      const p1 = projectA(a, a.kind.x1, a.kind.y1, t);
      const p2 = projectA(a, a.kind.x2, a.kind.y2, t);
      ctx.beginPath();
      ctx.moveTo(p1.x, p1.y);
      ctx.lineTo(p2.x, p2.y);
      ctx.stroke();
      ctx.restore();
      return;
    }

    const box = normaliseBox(a.kind);
    const tl = projectA(a, box.x, box.y, t);
    const br = projectA(a, box.x + box.w, box.y + box.h, t);
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
      if (sel && !sel.hidden) {
        drawSelection(ctx, sel, t);
        // Live dimensions while actively sizing (place/resize), not on idle
        // selection or plain moves, so the chip appears only when it helps.
        if (drag && (drag.kind === "place" || drag.kind === "resize") && drag.id === sel.id) {
          drawSizeBadge(ctx, sel, t);
        }
      }

      if (snapGuides.length > 0) drawSnapGuides(ctx, t);
    }
  }

  /** Draw the snap guides emitted during the active drag. Two guides max in
   *  practice (one per axis); the cap in `applySnap` enforces a hard ceiling. */
  function drawSnapGuides(ctx: CanvasRenderingContext2D, t: number) {
    const dpr = getDpr();
    // Guides live in the dragged annotation's space so they line up with it.
    const activeDrag = drag;
    const anchorObj =
      (activeDrag && store.annotations.find((x) => x.id === activeDrag.id)) || {};
    const r = rectFor(anchorObj);
    const zoom = zoomFor(anchorObj, t);
    if (r.w <= 0 || r.h <= 0) return;

    ctx.save();
    ctx.strokeStyle = selectionPalette().accentMuted;
    ctx.lineWidth = 1 * dpr;
    ctx.setLineDash([4 * dpr, 3 * dpr]);

    for (const g of snapGuides) {
      if (g.axis === "x") {
        const top = uvToCanvas(g.value, 0, r, zoom);
        const bot = uvToCanvas(g.value, 1, r, zoom);
        ctx.beginPath();
        ctx.moveTo(top.x, top.y);
        ctx.lineTo(bot.x, bot.y);
        ctx.stroke();
      } else {
        const left = uvToCanvas(0, g.value, r, zoom);
        const right = uvToCanvas(1, g.value, r, zoom);
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
    if (!canvasEl) return;
    // Fallback: if the cached size is still unknown (target not yet laid out
    // when the observer was set up), measure live so the canvas never gets
    // stuck at 1x1 — a 1x1 backing stretched over the preview renders the first
    // near-white handle as a full-screen white wash.
    if ((targetSize.w <= 0 || targetSize.h <= 0) && targetEl) {
      const r = targetEl.getBoundingClientRect();
      targetSize = { w: r.width, h: r.height };
    }
    const dpr = getDpr();
    const w = Math.max(1, Math.floor(targetSize.w * dpr));
    const h = Math.max(1, Math.floor(targetSize.h * dpr));
    if (canvasEl.width !== w || canvasEl.height !== h) {
      canvasEl.width = w;
      canvasEl.height = h;
    }
  }

  //  Pointer interaction

  function pickAnnotation(pt: { x: number; y: number }, t: number) {
    const dpr = getDpr();
    return hitTestAnnotation(pt, store.annotationsByZ, {
      project: (a, ux, uy) => projectA(a, ux, uy, t),
      t,
      handleSlop: HANDLE_RADIUS_PX * dpr + 2 * dpr,
      lineSlop: 6 * dpr,
      annotationSlop: 8 * dpr,
    });
  }

  function pickHandle(pt: { x: number; y: number }, a: Annotation, t: number) {
    const dpr = getDpr();
    return hitTestHandle(pt, a, {
      project: (anno, ux, uy) => projectA(anno, ux, uy, t),
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
        dragUndoPushed = false;
        e.preventDefault();
        return;
      }
      if (hit === "body") {
        // Body of the already-selected annotation → start moving immediately.
        // We deliberately skip the pickAnnotation path here so the annotation
        // can be moved during fade-in / fade-out windows where evalOpacity
        // would otherwise filter it out of the hit-test.
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        const pointerUV = unprojectA(selected, pt.x, pt.y, t);
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
        dragUndoPushed = false;
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
      const pointerUV = unprojectA(hitAnno, pt.x, pt.y, t);
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
      dragUndoPushed = false;
      e.preventDefault();
      return;
    }

    // No hit — if a tool is active, start placing a new annotation.
    const tool = store.annotationTool;
    if (tool) {
      // New annotations default to the video anchor.
      const anchor = unprojectA({}, pt.x, pt.y, t);
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
    const anchors = buildAnnotationSnapAnchors(store.annotations, dragId);
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

  function frameDims(): { w: number; h: number } {
    return { w: store.metadata?.width ?? 16, h: store.metadata?.height ?? 9 };
  }

  function handlePointerMove(e: PointerEvent) {
    if (!drag) {
      refreshHover(pointerToCanvasPx(e), playbackTime());
      return;
    }
    const pt = pointerToCanvasPx(e);
    const t = playbackTime();
    const f = frameDims();
    const dragAnno = store.annotations.find((x) => x.id === drag!.id) ?? {};
    const rawUv = unprojectA(dragAnno, pt.x, pt.y, t);
    // Alt held bypasses snap, matching Figma. Snap is per-axis so an annotation
    // can lock to a horizontal guide while still tracking the cursor vertically.
    const uv = applySnap(rawUv.x, rawUv.y, drag.id, e.altKey);

    // First real move of a move/resize commits one undo entry (placement
    // pushed at creation).
    if (!dragUndoPushed) {
      store.pushUndoState();
      dragUndoPushed = true;
    }

    if (drag.kind === "place") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      if (anno.kind.kind === "arrow") {
        const end = e.shiftKey
          ? constrain45(anno.kind.x1, anno.kind.y1, uv.x, uv.y, f.w, f.h)
          : { x: uv.x, y: uv.y };
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x2: end.x, y2: end.y },
        });
      } else if (
        anno.kind.kind === "rect" ||
        anno.kind.kind === "ellipse" ||
        anno.kind.kind === "text" ||
        anno.kind.kind === "image" ||
        anno.kind.kind === "blur"
      ) {
        let w = uv.x - drag.anchor.x;
        let h = uv.y - drag.anchor.y;
        if (e.shiftKey) ({ w, h } = constrainSquare(w, h, f.w, f.h));
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: drag.anchor.x, y: drag.anchor.y, w, h },
        });
      }
    } else if (drag.kind === "move") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      if (anno.kind.kind === "arrow") {
        const dx = uv.x - drag.pointerStartUV.x;
        const dy = uv.y - drag.pointerStartUV.y;
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
        // Snap the box's own edges/center to guides (not the raw cursor), so a
        // move aligns the annotation itself. Alt or the snap toggle bypasses.
        const rawDx = rawUv.x - drag.pointerStartUV.x;
        const rawDy = rawUv.y - drag.pointerStartUV.y;
        const bx = drag.startX + rawDx;
        const by = drag.startY + rawDy;
        const b = normaliseBox(anno.kind);
        let newX = bx;
        let newY = by;
        if (!e.altKey && store.annotationSnapEnabled) {
          const res = snapBox(bx, by, b.w, b.h, buildAnnotationSnapAnchors(store.annotations, drag.id), 0.005);
          newX = res.x;
          newY = res.y;
          snapGuides = res.guides.slice(0, 4);
        } else {
          snapGuides = [];
        }
        store.updateAnnotation(drag.id, {
          kind: { ...anno.kind, x: newX, y: newY },
        });
      }
    } else if (drag.kind === "resize") {
      const anno = store.annotations.find((a) => a.id === drag!.id);
      if (!anno) return;
      if (anno.kind.kind === "arrow") {
        if (drag.handle === "p1") {
          const p = e.shiftKey
            ? constrain45(anno.kind.x2, anno.kind.y2, uv.x, uv.y, f.w, f.h)
            : { x: uv.x, y: uv.y };
          store.updateAnnotation(drag.id, {
            kind: { ...anno.kind, x1: p.x, y1: p.y },
          });
        } else if (drag.handle === "p2") {
          const p = e.shiftKey
            ? constrain45(anno.kind.x1, anno.kind.y1, uv.x, uv.y, f.w, f.h)
            : { x: uv.x, y: uv.y };
          store.updateAnnotation(drag.id, {
            kind: { ...anno.kind, x2: p.x, y2: p.y },
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
      // Shift on a corner locks to the starting aspect ratio.
      if (e.shiftKey && isCornerHandle(h)) {
        ({ nx, ny, nw, nh } = lockAspect(h, b, nx, ny, nw, nh));
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
            // Cancelled placement: remove and unwind addAnnotation's undo push
            // so a stray click leaves no undo entries.
            store.removeAnnotation(drag.id, false);
            store.popUndoState();
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
            store.removeAnnotation(drag.id, false);
            store.popUndoState();
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
      a.kind.kind === "image" ||
      a.kind.kind === "blur"
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
      const selForNudge = store.annotations.find((x) => x.id === store.selectedAnnotationId) ?? {};
      const r = rectFor(selForNudge);
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
      // Coalesce a held/repeated arrow key into one undo entry (same key the
      // timeline layer card uses), so Ctrl+Z reverts the nudge, not an
      // unrelated earlier edit.
      store.pushUndoStateCoalesced(`nudge-annotation-${store.selectedAnnotationId}`, 600);
      nudgeBy(dx, dy);
    }
  }

  //  Lifecycle

  // Track the container size. A $effect (not onMount) so it re-establishes if
  // `targetEl` arrives after mount, and getBoundingClientRect (rendered size)
  // so a scaled/letterboxed preview maps to the right backing resolution.
  $effect(() => {
    const el = targetEl;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) targetSize = { w: r.width, h: r.height };
    };
    measure();
    const ro = new ResizeObserver(() => measure());
    ro.observe(el);
    return () => ro.disconnect();
  });

  onMount(() => {
    tick();
  });

  onDestroy(() => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle);
    disposeCanvasTokens();
  });

  const canvasCursor = $derived.by(() => {
    if (store.annotationTool) return "crosshair";
    if (drag?.kind === "move") return "grabbing";
    if (drag?.kind === "resize") return cursorForHandle(drag.handle);
    return cursorForHandle(hoverHandle);
  });
</script>

<!-- Local annotation editing keys (delete, deselect, Mod+D/[/], arrow nudge —
     documented in the central shortcut registry). `<svelte:window>` so HMR
     rebinds rather than leaks the listener. -->
<svelte:window onkeydown={handleKeyDown} />

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
