/**
 * Hybrid-raster export path for text annotations.
 *
 * The Rust export pipeline (cursor_export.rs / draw_annotation) does not know
 * how to render text — it has no font rasterizer. To keep the Rust side
 * simple, every text annotation is rendered to a transparent PNG by the
 * WebView (which already has full font support), then sent across IPC as a
 * data URL on a synthetic image-kind annotation.
 *
 * This module is invoked from the export trigger (handleExport) right before
 * the renderState payload reaches `invoke("export_video", ...)`. Non-text
 * annotations pass through untouched.
 */
import type { Annotation } from "$lib/stores/editor-store.svelte";

/**
 * Walk the annotations and replace every text annotation with an image
 * annotation whose `path` is a `data:image/png;base64,…` URL containing a
 * pre-rendered transparent PNG of the text.
 *
 * @param annotations  Annotation list as it would appear in toRenderState.
 * @param canvasWidth  Pixel width of the export canvas (source.width + 2*padding).
 * @param canvasHeight Pixel height of the export canvas.
 */
export async function expandTextAnnotations<
  T extends Pick<Annotation, "kind">,
>(annotations: T[], canvasWidth: number, canvasHeight: number): Promise<T[]> {
  if (canvasWidth <= 0 || canvasHeight <= 0) return annotations;
  const out: T[] = [];
  for (const a of annotations) {
    if (a.kind.kind !== "text") {
      out.push(a);
      continue;
    }
    const k = a.kind;
    const dataUrl = await renderTextToDataUrl(k, canvasWidth, canvasHeight);
    if (!dataUrl) {
      // Drop the annotation rather than fail the export; surface a console
      // hint so debug builds notice.
      console.warn(
        "rasterize-text: failed to render text annotation, skipping",
        k.content,
      );
      continue;
    }
    out.push({
      ...a,
      kind: {
        kind: "image",
        x: Math.min(k.x, k.x + k.w),
        y: Math.min(k.y, k.y + k.h),
        w: Math.abs(k.w),
        h: Math.abs(k.h),
        path: dataUrl,
        opacity: 1,
      },
    } as T);
  }
  return out;
}

async function renderTextToDataUrl(
  k: Extract<Annotation["kind"], { kind: "text" }>,
  canvasWidth: number,
  canvasHeight: number,
): Promise<string | null> {
  // UV box → export-canvas pixels.
  const boxW = Math.max(1, Math.round(Math.abs(k.w) * canvasWidth));
  const boxH = Math.max(1, Math.round(Math.abs(k.h) * canvasHeight));
  const fontPx = Math.max(1, Math.round(k.fontSize * canvasHeight));

  const canvas = document.createElement("canvas");
  canvas.width = boxW;
  canvas.height = boxH;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  ctx.clearRect(0, 0, boxW, boxH);
  ctx.font = `${k.fontWeight} ${fontPx}px ${k.fontFamily}`;
  ctx.fillStyle = k.color;
  ctx.textBaseline = "top";
  ctx.textAlign =
    k.align === "center" ? "center" : k.align === "right" ? "right" : "left";

  const lines = wrapText(ctx, k.content, boxW);
  const lineHeightPx = fontPx * Math.max(1, k.lineHeight);
  const xAnchor =
    k.align === "center" ? boxW / 2 : k.align === "right" ? boxW - 1 : 0;

  for (let i = 0; i < lines.length; i++) {
    const y = i * lineHeightPx;
    if (y + lineHeightPx > boxH + lineHeightPx) break;
    ctx.fillText(lines[i], xAnchor, y);
  }

  return canvas.toDataURL("image/png");
}

/**
 * Greedy word-wrap that respects explicit "\n" line breaks. Handles the case
 * where a single word is wider than `maxWidth` by emitting it on its own line
 * without splitting characters.
 */
function wrapText(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split(/\r?\n/)) {
    if (paragraph.length === 0) {
      lines.push("");
      continue;
    }
    const words = paragraph.split(/\s+/);
    let current = "";
    for (const word of words) {
      const candidate = current ? `${current} ${word}` : word;
      if (ctx.measureText(candidate).width <= maxWidth || current === "") {
        current = candidate;
      } else {
        lines.push(current);
        current = word;
      }
    }
    if (current) lines.push(current);
  }
  return lines.length > 0 ? lines : [""];
}
