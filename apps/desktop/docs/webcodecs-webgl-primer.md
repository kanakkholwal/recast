# WebCodecs and WebGL primer

Background reading, not architecture. What Recast does with these APIs lives at
[/architecture](https://recast.li/architecture); this page is the mental model
you need before that makes sense.

## 4. WebCodecs primer (since you said it's new)

**What it is.** A browser API for decoding audio and video *without* the
`<video>` element. You hand it encoded bytes (e.g. h.264 NAL units from an
MP4), it gives you back raw frames (`VideoFrame`) you can upload to a canvas
or a `OffscreenCanvas`. It runs on a worker thread, off the main JS
thread, so the UI never blocks.

**Why use it instead of `<video>`.** Two reasons:

1. **Sub-frame seek latency.** A `<video>` element's `currentTime = t` is
   a "go to the nearest keyframe, decode forward to t" operation you can't
   control the speed of. A WebCodecs decoder resets in microseconds and
   decodes from a keyframe you choose yourself. For a non-linear editor
   with cuts every few seconds, `<video>` is visibly broken; WebCodecs is
   fine.
2. **No black box around the decoder.** You own the encoded bytes; you can
   edit the bitstream if you ever need to (we don't, yet). The `<video>`
   element is a fixed pipeline.

**The shape of a `VideoFrame`.** A `VideoFrame` is a handle to a GPU-backed
texture. You don't read its bytes; you *upload* it to a canvas with
`gl.texImage2D(target, level, internalformat, format, type, videoFrame)`. The
GPU does the copy. Each `VideoFrame` reserves one of the decoder's limited
output surfaces — typically 8 to 16 — and leaking them starves the decoder
into "8 fps and stalling" hell. Always close them (`frame.close()`) when
done with one.

**We don't call WebCodecs directly.** MediaBunny (`@recast/media`) wraps it.
We hand it a URL; it demuxes (mp4/mov/webm), configures the `VideoDecoder`,
and answers "give me the frame at time T" via a `CanvasSink`. The
hand-rolled demuxer + sample table this section used to describe was
deleted when the preview moved to MediaBunny.

**The work split (look at `mediabunny-source.ts` and `mediabunny-worker.ts`):**

```
┌─ Main thread (MediabunnyVideoSource) ────────────────────────┐
│                                                              │
│  • owns the bounded decoded-frame cache (Map<tsUs, VideoFrame>)│
│  • answers `frameAt(originalSec)` synchronously for the        │
│    render loop's `requestAnimationFrame` callback              │
│  • supersedes stale in-flight seeks (#inFlightSeq guard)       │
│  • talks to the worker over postMessage                        │
│                                                              │
└──────────────────────────┬───────────────────────────────────┘
                           │  postMessage
┌─ Worker thread (mediabunny-worker.ts) ────────────────────────┐
│                                                              │
│  • MediaBunny `Input` demuxes the file (mp4/mov/webm)          │
│  • MediaBunny `CanvasSink` answers any timestamp deterministically
│  • MediaBunny owns the `VideoDecoder` + keyframe seek internally
│  • posts decoded frames back to main, tagged with the seek seq  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**Why a worker?** The `VideoDecoder` itself runs on a worker thread
internally (it's specified that way), but the demux step and the
seek-orchestration logic are CPU-bound JS that's worth moving off the
main thread. The main thread is left with only the GPU upload + the cache,
which is the only thing that has to be on the main thread to render.

**Ingestion strategy.** MediaBunny's `Input` decides this itself from the
source adapter it's handed — desktop passes a Tauri `asset:` URL, web
passes a range-capable HTTP URL. The old whole-file-vs-progressive
`chooseIngestion` heuristic is gone; MediaBunny range-reads as the
playhead moves. Either way the `MediabunnyVideoSource` interface is
identical; the rest of the editor doesn't care.

**The gap.** MediaBunny cannot decode AVI, FLV, WMV, RealVideo, or 3GP
(neither could the old pipeline). Those fall back to the `<video>`
element. The list is curated and tested in
`packages/media/src/cache/unsupported-formats.ts`.

> **Read these files in order:**
>
> 1. `packages/media/src/playback.ts` (the `PlaybackSource` interface)
> 2. `packages/media/src/playback/worker.ts` (the decode worker)
> 3. `packages/media/src/playback/source.ts` (the main-thread proxy)
> 4. `packages/media/src/cache/frame-budget.ts` (resolution-adaptive cache sizes)
> 5. `packages/media/src/cache/index.ts` (the frame cache + `readNearest`)

---

## 5. WebGL primer (since you said it's new)

**What it is.** A browser API for drawing 2D and 3D to a canvas with
hardware acceleration. We use **WebGL2** (the modern version) for the
video preview compositor.

**Mental model.** A WebGL program is two compiled shaders (vertex + fragment)
plus a state machine. The vertex shader runs per-vertex; the fragment
shader runs per-pixel of the primitive being drawn. The state machine is a
bag of GPU resources: buffers (vertex data), textures (images), framebuffers
(offscreen render targets), and uniform variables (per-draw constants).

**What we use it for.** One thing: blit a `VideoFrame` to the canvas with
a zoom transform applied. The shader takes a `vec2` of UV coordinates
(0..1) and a `uniform float scale` plus a `uniform vec2 center`, and does
something like:

```glsl
// vertex: pass UVs through
out vec2 v_uv = a_uv;

// fragment: sample the video frame at the inverse-zoomed UV
in vec2 v_uv;
uniform sampler2D u_video;
uniform float u_scale;   // >1 = zoomed in
uniform vec2  u_center; // 0.5, 0.5 = center of the frame

void main() {
  // Affine zoom: pull the UV toward the center by 1/scale, then re-anchor.
  vec2 uv = (v_uv - u_center) / u_scale + u_center;
  // (u_center fixed for the whole region; only the scale eases. See
  //  video-preview.logic.ts:65-71 for why the centre is constant.)
  gl_FragColor = texture(u_video, uv);
}
```

That's the whole thing. There's also a corner-rounding pass, a
background-blur pass (for letterboxing), and a cursor sprite, all on the same
canvas. The point of WebGL isn't the complexity — it's that the
`texImage2D(target, internalformat, format, type, videoFrame)` upload path
zero-copies the `VideoFrame` from the decoder's output surface to our
texture. No CPU readback.

**Helper module.** `webgl.logic.ts` is a 40-line file with two functions
(`compile`, `link`) that wrap shader creation. That's all you need to get
started; everything else (uniforms, attributes, framebuffers) is
straightforward WebGL2.

**Why not WebGPU?** Two reasons:
1. WebGPU is in some browsers but not all (Tauri uses the system WebView,
   which is platform-variable).
2. WebGL2 has *zero-copy video frame upload* in Chromium-based WebViews,
   which is what we need. WebGPU's video frame path is more verbose and
   not measurably faster for our 60 fps preview use case.

If/when we need compute shaders (e.g. for real-time cursor compositing) we
might add WebGPU behind a feature flag.

> **Read these files in order:**
> 1. `apps/desktop/src/components/editor/webgl.logic.ts` (the helpers)
> 2. `apps/desktop/src/components/editor/VideoPreview.svelte` (the canvas + the upload)
> 3. `apps/desktop/src/components/editor/video-preview.shaders.ts` (the actual GLSL)

---
