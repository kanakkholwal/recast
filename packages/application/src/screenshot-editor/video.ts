import { domToCanvas } from "modern-screenshot";
import { ArrayBufferTarget, Muxer } from "mp4-muxer";
import { type AnimationPreset, propsAtTime, propsToTransform } from "./animation";
import { exportFilter } from "./export";

/** True when this browser can encode H.264 via WebCodecs. */
export function canExportVideo(): boolean {
  return typeof VideoEncoder !== "undefined" && typeof VideoFrame !== "undefined";
}

/** Drive the framed content's live transform to a given animation time by
 * mutating the same DOM nodes the preview uses, so the captured frame is
 * exactly what the preview shows. */
function applyFrame(persp: HTMLElement, tilt: HTMLElement, preset: AnimationPreset, time: number) {
  const p = propsAtTime(preset, time);
  persp.style.perspective = `${p.perspective}px`;
  tilt.style.transform = propsToTransform(p);
  tilt.style.opacity = String(p.opacity);
}

/**
 * Encode the selected motion preset to an MP4 by snapshotting the real stage at
 * each frame (so 3D perspective, mockups, and overlays all render faithfully),
 * then muxing the WebCodecs H.264 chunks. `onProgress` reports 0..1.
 */
export async function exportVideo(
  stage: HTMLElement,
  preset: AnimationPreset,
  fps: number,
  onProgress?: (progress: number) => void,
  /** Wall-clock length of the clip; defaults to the preset's natural duration.
   * A stretched timeline clip plays the same motion over a longer span. */
  durationMs?: number,
): Promise<Blob> {
  if (!canExportVideo()) throw new Error("this browser can't encode video (needs WebCodecs)");

  const persp = stage.querySelector<HTMLElement>(".recast-shot-persp");
  const tilt = stage.querySelector<HTMLElement>(".recast-shot-tilt");
  if (!persp || !tilt) throw new Error("load an image before exporting a clip");

  // Preserve the live styles so the editor is untouched afterwards.
  const saved = {
    perspective: persp.style.perspective,
    transform: tilt.style.transform,
    opacity: tilt.style.opacity,
    transition: tilt.style.transition,
  };
  tilt.style.transition = "none";

  // Declared out here so `finally` can close it: a VideoEncoder holds a scarce
  // hardware encoder session, and every throw path below (domToCanvas, encode,
  // flush) used to leak one for the life of the process.
  let encoder: VideoEncoder | null = null;

  try {
    // Probe frame 0 to size the output; force even dims, cap the long edge.
    applyFrame(persp, tilt, preset, 0);
    const probe = await domToCanvas(stage, { scale: 2, filter: exportFilter });
    const cap = 1920;
    const longest = Math.max(probe.width, probe.height);
    const f = longest > cap ? cap / longest : 1;
    const width = Math.max(2, Math.round(probe.width * f)) & ~1;
    const height = Math.max(2, Math.round(probe.height * f)) & ~1;

    const out = document.createElement("canvas");
    out.width = width;
    out.height = height;
    const octx = out.getContext("2d");
    if (!octx) throw new Error("could not create a drawing context");

    const muxer = new Muxer({
      target: new ArrayBufferTarget(),
      video: { codec: "avc", width, height },
      fastStart: "in-memory",
    });
    encoder = new VideoEncoder({
      output: (chunk, meta) => muxer.addVideoChunk(chunk, meta),
      error: (e) => {
        throw e;
      },
    });
    encoder.configure({ codec: "avc1.42001f", width, height, bitrate: 6_000_000, framerate: fps });

    // Frame count comes from the clip's wall-clock length; the motion itself is
    // always sampled across the preset's full range, so a stretched clip plays
    // the same animation more slowly.
    const clipMs = durationMs && durationMs > 0 ? durationMs : preset.duration;
    const total = Math.max(2, Math.round((clipMs / 1000) * fps));
    for (let i = 0; i < total; i++) {
      const time = (i / (total - 1)) * preset.duration;
      applyFrame(persp, tilt, preset, time);
      const canvas = await domToCanvas(stage, { scale: 2, filter: exportFilter });
      octx.clearRect(0, 0, width, height);
      octx.drawImage(canvas, 0, 0, width, height);
      const frame = new VideoFrame(out, {
        timestamp: Math.round((i * 1_000_000) / fps),
        duration: Math.round(1_000_000 / fps),
      });
      encoder.encode(frame, { keyFrame: i % fps === 0 });
      frame.close();
      onProgress?.((i + 1) / total);
    }

    await encoder.flush();
    muxer.finalize();
    return new Blob([muxer.target.buffer], { type: "video/mp4" });
  } finally {
    // `close()` on an already-closed encoder throws, and flush() succeeding
    // still leaves it open.
    if (encoder && encoder.state !== "closed") encoder.close();
    persp.style.perspective = saved.perspective;
    tilt.style.transform = saved.transform;
    tilt.style.opacity = saved.opacity;
    tilt.style.transition = saved.transition;
  }
}
