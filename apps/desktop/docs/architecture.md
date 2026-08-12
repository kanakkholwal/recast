# Recast — System Architecture

A practical map of how every piece fits together: the recording pipeline, the
non-linear editor, the WebCodecs/WebGL preview, the Rust export, and the Tauri
IPC that holds the two sides in sync.

> **Who this is for.** Engineers joining the project who know some Rust and some
> TypeScript/Svelte, but who may not have touched `WebCodecs` or `WebGL` before.
> Read top-to-bottom once, then keep it open as a reference while reading
> individual modules — every section ends with a "go look at" pointer.

---

## 0. The one-paragraph version

A recording is captured by Rust threads (screen grabber + mic + cursor sampler)
into a 60 fps BGRA queue, then H.264 encoded to an MP4 by an FFmpeg subprocess.
A project on disk is a JSON "edit graph" that says "trim head/tail, cut these
ranges, apply these zoom regions, smooth the cursor, render to a final canvas
of this size". The editor is a Svelte app that loads the recording into a
WebCodecs decoder running in a Worker, plays it back through a single
WebGL2 canvas using `texImage2D(upload, videoFrame)`, and lets the user reshape
it. Every preview frame goes through the same maths the Rust exporter uses, so
"what you see is what gets rendered". The Tauri shell hosts the WebView, owns
filesystem + tray + hotkeys + screen capture, and exposes async commands to
JS via `invoke()`.

---

## 1. Monorepo layout

```
recast/
├── apps/
│   ├── desktop/                       Tauri v2 desktop app
│   │   ├── src/                       Svelte 5 frontend (the editor UI)
│   │   │   ├── lib/
│   │   │   │   ├── timeline/          Cuts, segments, time map, storyboard
│   │   │   │   ├── playback/          Clock, A/V sync, Web Audio engine
│   │   │   │   ├── render/            Pure preview maths (matches Rust export)
│   │   │   │   ├── editor/            Time formatting, frame padding
│   │   │   │   ├── stores/            Svelte 5 runes-based state containers
│   │   │   │   ├── components/editor/ Svelte components (Timeline, VideoPreview…)
│   │   │   │   └── services/           Thin wrappers over Tauri `invoke()`
│   │   │   └── routes/                SvelteKit file-based routing (SPA mode)
│   │   └── src-tauri/                 Rust backend
│   │       ├── src/
│   │       │   ├── recording/         Screen-capture pipeline (xcap → ffmpeg)
│   │       │   ├── encoder/           H.264 encoder (subprocess to ffmpeg)
│   │       │   ├── render/            Export compositor (FFmpeg filter graph)
│   │       │   ├── commands/           Tauri commands (the JS↔Rust bridge)
│   │       │   ├── transcription/     on-device captioning (ggml / whisper)
│   │       │   ├── project/           `.recast` project files on disk
│   │       │   └── …                   (capture, audio, ocr, cursor, jumplist…)
│   │   └── docs/                       Design docs (this file lands here)
│   └── web/                            SvelteKit marketing / docs site
├── packages/                           Shared workspace libraries
│   ├── media/                          MediaBunny wrapper: decode worker,
│   │                                    frame cache, conversion, audio
│   ├── application/                    Store + service layer (TS)
│   ├── player/                         Standalone video player (TS)
│   ├── captions/                       SRT/VTT writer + caption model
│   ├── design/                         Tailwind theme tokens
│   ├── analytics/                      PostHog-style wrapper around `analytics/`
│   └── ui/                             shadcn-svelte primitives
├── apps/desktop/src-tauri/Cargo.toml   Single Rust crate (`recast` lib + bins)
└── package.json                       pnpm workspace
```

The Svelte frontend and the Rust backend are the same binary: `tauri build`
produces a native installer that embeds a WebView, copies a FFmpeg sidecar
beside it, and the WebView loads the Svelte app. There is no separate
"backend process" — `tauri-plugin-single-instance` is what stops you from
running two of them at once.

---

## 2. The recording pipeline (Rust)

**What runs where.** All in-process, all spawned by `RecordingManager::start`
in `apps/desktop/src-tauri/src/recording/mod.rs`.

```
            ┌─────────────────────────┐
60 fps      │  Capture thread          │  BGRA frames in a bounded
paced       │  (xcap::Monitor::capture) │  crossbeam ArrayQueue
pacer ────►│  per monitor OR per      │  (capacity sized to a
            │  window handle           │  256 MB BGRA budget)
            └────────────┬────────────┘
                         │ Vec<u8> + (w, h, ts_us)
                         ▼
            ┌─────────────────────────┐
            │  Encoder thread          │  spawn ffmpeg subprocess
            │                          │  (h264_nvenc / h264_qsv /
            │                          │  libx264 software fallback)
            └────────────┬────────────┘
                         │ MP4 mux
                         ▼
            ┌─────────────────────────┐
            │  Cursor + Mic threads   │  Independent tracks written
            │  (separately timestamped)│  to the same MP4 mux as
            │                          │  data streams
            └────────────┬────────────┘
                         ▼
                  final .mp4 in <output>/exports/
```

**Time invariants.** Everything is anchored to one wall-clock `Instant` in
`RecordingClock`:

- The pacer emits exactly `RECORDING_FPS = 60` frames per real-time second.
- The encoder's `-framerate 60` produces matching PTS.
- The cursor and mic tracks are timestamped in **the same** wall-clock µs.

Pause is not a gap; it's a hidden counter. `RecordingClock::pause()`
records `(now - start)` into a `paused_total_us` atomic. `effective_elapsed()`
subtracts that from `start.elapsed()`. **Every** downstream consumer (pacer,
encoder, cursor) reads `effective_elapsed()`, so a paused span never appears
in any track. The recording stays one gap-free timeline; cuts in the editor
are a *separate* concern that live in the JSON edit graph.

**The dropped-frame path.** If the encoder falls behind the pacer, the
bounded queue refuses a push and increments `dropped_frames`. A loud warn
fires on the first drop of each session, then a dampened "N frames dropped
total" every 5 seconds. Don't ship encoder-tuned builds that drop on a
default recording; that's the *user-visible* "choppy" bug.

> **Read these files in order** to internalize the recording side:
> 1. `apps/desktop/src-tauri/src/recording/mod.rs` (the state machine)
> 2. `apps/desktop/src-tauri/src/recording/pipeline.rs` (the queue)
> 3. `apps/desktop/src-tauri/src/encoder/h264.rs` (the FFmpeg subprocess)
> 4. `apps/desktop/src-tauri/src/capture/` (xcap wrappers)

---

## 3. The non-linear editor — timeline data model (frontend)

This is the most subtle part of the codebase. Read it twice.

### 3.1 Two time axes, one display

The user sees ONE time: the **output** time, post-cuts, the time that gets
exported. Internally there are TWO:

- **Original time** — the same coordinate the recording uses. Zoom
  regions, annotations, and the cursor samples are all in original time.
- **Output time** — original time minus all `Cut` ranges. The playhead,
  the timeline ruler, the transport, the export — all output time.

A `TimelineCut` is just `{ id, start, end, source: "silence" | "manual" }`
in **original** seconds. There is no "output-time cut" — cuts only exist as
removal ranges in the original axis, and the time-map is what collapses them
to a shorter output axis. One source of truth.

> **Read `apps/desktop/src/lib/timeline/cuts.ts` first.** It is 100 lines,
> pure, fully unit-tested, and is the single arithmetic source for
> `originalToOutput` / `outputToOriginal` everywhere else.

### 3.2 The three layers

```
            ┌─────────────────────────────────────────────┐
            │  ClipShape                                    │  The "what is the user
            │  ─────────                                   │  editing" view:
            │  trimStart, trimEnd,                          │  outer trim, removed
            │  cuts: TimelineCut[],                         │  ranges, split points.
            │  splitPoints: number[]                         │  Pure data.
            └─────────────────┬───────────────────────────┘
                              │  deriveSegments()
                              ▼
            ┌─────────────────────────────────────────────┐
            │  Segment[] (kept, ordered)                  │  The "what plays
            │  ─────────                                   │  back" view.
            │  index, start, end                            │  One per kept span
            │                                               │  (post-cut) split by
            │                                               │  splitPoints.
            └─────────────────┬───────────────────────────┘
                              │  + per-segment speed
                              ▼
            ┌─────────────────────────────────────────────┐
            │  TimeMap                                     │  The "what maps to
            │  ─────────                                   │  what" view:
            │  spans: MappedSpan[]                          │  orig↔output with
            │  outputDuration                              │  per-segment slope.
            └─────────────────────────────────────────────┘
```

**`ClipShape` is what the user manipulates.** They drag the trim handles
(moving `trimStart`/`trimEnd`), drop silence cuts, mark split points, delete
segments. None of these touches the `TimeMap` directly.

**`Segment[]` is what plays.** It's always derived — you can't construct a
Segment by hand and have it stick. The derive drops:
- spans outside the trim (`trimStart, trimEnd`),
- spans inside any cut (removed by the cuts overlay),
- zero-length slices from cuts coincident with split points,
- any split point outside the clip or inside a cut.

**`TimeMap` is the playhead's coordinate system.** It annotates each
Segment with its `outStart`/`outEnd` and the speed. The playhead advances
in output time (`PlaybackClock` is in output time); we call
`outputToOriginal(map, t)` to ask the video source for the frame to show.

**The parity rule.** Preview math (`video-preview.logic.ts`, cursor
interpolation, time formatting in `editor/time.ts`) and the Rust export
(`render/graph.rs`, `render/cursor_export.rs`, `render/scene_anim.rs`) MUST
agree on the same maths. They share the segments model: both apply
`outputToOriginal` to map playhead to original time. If you change one side,
you change the other. **Always run `cargo test --release -p recast` after
editing either side.** There are shared `time-map.test.ts` ↔ Rust parity tests
that catch divergence.

### 3.3 Speed per segment (the per-clip-variable-playback-rate)

Each Segment has a `speed` (1× default). The TimeMap builds a piecewise
linear function `original → output` where each kept span has slope
`1/speed`. `originalToOutput` for a time inside a span is
`outStart + (t - origStart) / speed`. So a 10-second kept segment at 2×
speed becomes 5 seconds of output; the playhead visits that span at half
real-time (the recording wall clock and the output clock diverge during it).

With every span at 1×, this reduces **exactly** to the simpler cut-only map,
which is the proven-by-tests baseline. The `time-map.test.ts` ↔ Rust tests
enforce that.

### 3.4 Scrubbing

Scrubbing is a seek: the playhead jumps to a new output time, the video
source tears down, re-decodes from the previous keyframe, and the render loop
keeps asking `source.frameAt(originalSec)` every frame. With WebCodecs
this is sub-frame; with the legacy `<video>` element seek it would freeze
for ~50 ms per cut (the *original* "playback freezes at a cut" bug — see
the header comment in `packages/media/src/playback/source.ts`).

> **Read these files to internalize the timeline model:**
> 1. `apps/desktop/src/lib/timeline/cuts.ts` (the arithmetic)
> 2. `apps/desktop/src/lib/timeline/segments.ts` (deriveSegments)
> 3. `apps/desktop/src/lib/timeline/time-map.ts` (per-segment speed)
> 4. `apps/desktop/src/lib/timeline/storyboard.ts` (sprite grid for the hover thumbnail)
> 5. `apps/desktop/src/lib/timeline/segment-speed.ts` (split-point vs. speed)

---

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

## 6. The editor's render loop (frontend, all together)

```
    requestAnimationFrame(t0)
        │
        ▼
    PlaybackClock.time       ←  where the playhead "is" right now
        │  (output seconds)
        ▼
    outputToOriginal(map, t)   ←  what frame to show
        │  (original seconds)
        ▼
    webcodecsSource.frameAt(orig)   ←  the cached VideoFrame
        │  (a GPU-backed handle)
        ▼
    gl.texImage2D(…, videoFrame)   ←  upload to our texture
        │
        ▼
    shader pass with current zoom + cursor + background
        │
        ▼
    gl.drawArrays(TRIANGLE_STRIP, 0, 4)
```

The loop is a single `requestAnimationFrame` callback in
`VideoPreview.svelte`. Every visible frame walks this graph. The
`VideoFrame` is owned by the cache; we upload it but never close it (the
source evicts it on the next call to `frameAt`).

**Why this loop lives in the same module as the canvas.** The
`VideoPreview.svelte` component owns the `WebGL2RenderingContext` (one per
mount). All the `gl.*` calls happen in this file. The pure-parity math
(`video-preview.logic.ts`) is separate and unit-tested without a real GL
context — that's the split that lets preview and Rust export stay
1:1-aligned.

**Pause + scrub.** Both are just "stop the rAF loop" + "tell the source to
seek". The clock handles pause as a wall-clock anchor (`anchorTime`,
`anchorWallMs`); seek is a re-anchor to a new `t`. See `playback/clock.ts`.

---

## 7. Tauri IPC — the bridge

The Svelte frontend calls into Rust via `invoke('command-name', args)`. The
Rust side declares them with `#[tauri::command]`. Each command is an
**async** function on the Rust side; the JS side gets a `Promise<T>` back.
Long-running commands (`start_recording`, `enqueue_export`) take seconds
to minutes; they're `spawn_blocking` internally so the main thread stays
free (this is the rule from `apps/desktop/src-tauri/src/lib.rs`).

**The contract.**
- All payloads cross as **JSON**, via `serde::Serialize` on the Rust side
  and `JSON.stringify` on the JS side.
- All IPC structs are `#[serde(rename_all = "camelCase")]` so the
  field-name contract is Rust-snake ↔ JS-camel.
- All Tauri commands are *thin IPC adapters*: they deserialize the args,
  call into a service module (`crate::commands::editor::*`, `recording::*`),
  serialize the result, and return. No business logic in the command body.

> **A common bug pattern.** Putting FFmpeg argv-building in the Tauri
> command. Don't. The command should call a service module that does the
> work, so the work is testable from a Rust unit test, and so the same
> service module can be called from `cargo test` or from the CLI bridge.
> See `apps/desktop/src-tauri/src/lib.rs:140-150` for the rule.

**Bidirectional events.** The Rust side can `emit("topic", payload)` to the
JS side. The JS side subscribes via `listen('topic', handler)`. Examples:
`tray:record-toggle` (tray → frontend), `updater:check-from-tray` (tray →
frontend to show the corner card), `updater:available` (Rust updater →
frontend), and the per-`render` `frame` events from the camera/webcam
preview.

> **The bridge is async + bidirectional, but it's not a streaming channel.**
> Don't try to use it for high-frequency data (per-frame video data, for
> example). That goes through the WebView's own mechanisms: WebCodecs ↔
> WebGL, IPC for "user pressed pause" only.

---

## 8. Recording → edit → export: the full pipeline

```
1. RECORDING
   Rust: xcap.grab_frame()    → 60 fps BGRA queue
   Rust: encoder ffmpeg subprocess → MP4 file in <output>/exports/

2. LOAD IN EDITOR
   JS:   fetch(recastUrl) via asset://
   JS:   createMediabunnySource(url)   ($lib/playback/mediabunny — owns the
           │                            worker spawn; the package never does)
           ├─ MediaBunny Input demuxes the file
           ├─ CanvasSink answers any timestamp
           ├─ worker range-reads as needed
           └─ main thread: bounded LRU frame cache (512 MB, frames closed on evict)
   Svelte: build initial RenderState
            { trim, cuts, zoom_regions, cursor_samples, ... }
            from the auto-detected silence cuts (silence detection
            runs in the same Rust pipeline right after recording)

3. EDIT (the user does this in the UI)
   Svelte: <Timeline> renders the ClipShape
            <VideoPreview> renders the current frame at every rAF
   User:  drags trim handles → updates trimStart/trimEnd
          drops silence cuts → updates cuts
          adds zoom regions → updates zoom_regions
          adds annotations → updates annotations
          sets cursor smoothing → updates cursor_smoothing
   All edits are committed to a Svelte 5 runes-based store
   (lib/stores/editor-store.svelte.ts). The store is the single source
   of UI-side state; the backend doesn't see it.

4. PREVIEW
   rAF loop walks the diagram in §6 above
   Every visible frame goes through the same math as the export
   See video-preview.logic.ts:5-7 for the parity contract.

5. EXPORT
   JS:   invoke('enqueue_export', { req })
   Rust: commands/export_queue.rs enqueues the job, then `run_export_job`
   Rust: spawn_blocking a worker thread that:
     a. constructs an FFmpeg filter graph from RenderState
        (see apps/desktop/src-tauri/src/render/mod.rs)
     b. applies the cut/trim map: `select='not(between(t,0,trimStart)
        +between(t,trimEnd,duration))'` plus per-cut `aselect`
     c. composites the cursor track onto the video (cursor_export.rs)
     d. applies per-segment speed via `setpts=N/(TB*speed)` per kept span
     e. rasterizes annotations to PNGs, overlays them via `-i` + `overlay`
     f. applies the camera overlay (zoom region) via crop + scale
     g. muxes to MP4 / WebM / GIF per the format flag
   Rust: emit('export:progress', { pct, eta })
   JS:   <ExportPanel> shows the progress bar

6. RENDERED FILE LANDS IN <output>/exports/
```

The export is the most CPU-intensive part of the whole pipeline. It runs
on a `spawn_blocking` worker so the main thread (and therefore the JS
side, the tray, the global hotkeys) stays free. **Do not** do the export
inline in a Tauri command — the macOS WKWebView will freeze, the
Windows WebView2 will throw a watchdog error, and the user will see "this
app hangs" and quit.

> **The cut/trim map.** This is the trickiest part of the export and the
> one most likely to break when you change the timeline model. Read
> `apps/desktop/src-tauri/src/render/mod.rs` carefully. The
> `select`/`aselect` filters are the inverse of the JS-side
> `originalToOutput`. They MUST agree. Run the existing
> `render::export_parity_test.rs` to confirm.

---

## 9. Tauri shell, hotkeys, tray, permissions

These are all the "this is a real app" bits that aren't about editing.

- **Tray** (`apps/desktop/src-tauri/src/tray.rs`): see recent commit, has
  status header, recording control, output access, recent exports,
  recent projects, about submenu, quit. Built in `setup()` after
  `app.manage(AppState)`. Uses `tauri-plugin-tray-icon`. Tooltip updates
  on recording state.

- **Global hotkeys** (Alt+Shift+R / Alt+Shift+P): `tauri-plugin-global-shortcut`
  in `lib.rs`. The handler emits `tray:record-toggle` /
  `global-shortcut:launch-panel` / `tray:pause-toggle` to the frontend,
  which routes them through the same code path as the tray menu. The
  registration is RAII-managed via `AppState.registered_shortcuts: Mutex<Vec<Shortcut>>`
  — `Run` closure unregisters on `Exit`/`ExitRequested` so a force-killed
  process doesn't leave a stale OS-level hotkey bound.

- **Single-instance** (`tauri-plugin-single-instance`): the first process
  to launch holds the OS mutex. Subsequent `recast.exe` invocations detect
  this, fire the handler (which shows the existing window + forwards argv),
  and exit. **Dev (`cargo tauri dev`) builds skip the plugin entirely**
  (cfg-gated in `install_singleton_plugin` at `lib.rs:130-160`) so a
  developer's running iteration doesn't immediately forward its argv to
  the installed production binary.

- **Permissions** (first-launch flow):
  - macOS: `nsevents` for global hotkey, ScreenCaptureKit for screen
    recording, AVAudioSession for mic, AVFoundation for camera. Tauri
    prompts on first use; we don't show a custom dialog.
  - Windows: `dxgi` for screen, `wasapi` for mic, `mfreadwrite` for
    camera. Tauri shows the OS dialog.
  - Linux: `x11` (XComposite / XDamage) for screen via `xcap`, PulseAudio
    for mic, v4l2 for camera, PipeWire (recommended) or PulseAudio for
    system audio. Wayland support is in-progress.

- **Permissions required (for the user / release notes):**

  | OS    | What                                                              | Why                                        |
  |-------|-------------------------------------------------------------------|--------------------------------------------|
  | macOS | Screen Recording, Microphone, Camera                              | recording, voiceover, webcam                |
  | Win   | same as macOS                                                      | same                                       |
  | Linux | X11 / Wayland screen, mic via PulseAudio or PipeWire, camera via v4l2 | same                            |

  These are gated behind the same Tauri permissions model as recording.

---

## 10. Concrete "what happens when" walkthroughs

### 10.1 User clicks "Start Recording"

1. `CaptionsPanel.svelte` `on:click={startRecording}`
2. `invoke('start_recording', { intent })` (an `Intent` in
   `commands/intent.rs`)
3. `start_recording` Tauri command: validates the intent, calls
   `RecordingManager::start()`, which:
   a. spawns the capture thread (xcap, BGRA, 60 fps)
   b. spawns the cursor sampler thread (`cursor::spawn_cursor_capture`)
   c. spawns the audio capture thread (`audio/`)
   d. spawns the encoder subprocess (ffmpeg + h264_nvenc / libx264)
4. All four threads are RAII-tied to the `RecordingManager` via `JoinHandle`
   stored in the manager. On `stop_recording` they're joined in order.
5. `RecordingClock` ticks; the cursor and audio tracks read
   `effective_elapsed()` for their timestamps; the pacer reads it to
   pace the 60 fps emission.
6. After stop: the `Recording` struct is saved to the project file with
   the auto-detected silence cuts pre-populated (so the editor opens with
   suggestions, not a blank timeline).

### 10.2 User scrubs the playhead to a cut

1. `Timeline.svelte` `on:click` on a ruler tick → `editorStore.seek(t)`
2. Store sets `clock.anchorTime = t; clock.playing = false`
3. `VideoPreview`'s rAF callback reads `clock.time = t`
4. Calls `outputToOriginal(map, t)` to find the original-time frame
5. Calls `mbSource.frameAt(originalSec)`:
   - main thread: emits `{ type: 'seek', seq, originalSec }` to worker
   - worker: MediaBunny's CanvasSink seeks to the nearest keyframe and
     decodes forward, posting the frame back as an OffscreenCanvas
   - main thread: returns the closest in-cache frame
6. `gl.texImage2D(..., videoFrame)` uploads
7. Shader renders
8. If `t` is in a removed cut, the source returns null and the preview
   holds the previous frame. The playhead is in a cut; nothing to show.

**Why no `<video>`-element seek latency.** With WebCodecs (via
MediaBunny), the
worst-case scrub latency is "the time to decode one GOP from the
nearest keyframe", which on a 60 fps clip is ~17 ms × GOP size. With
`<video>`, the seek latency is whatever Chromium / WebKit / WebView2
decide it is, which on a 4K clip can be 100+ ms. That's the visible
freeze at every cut that the WebCodecs path eliminates.

### 10.3 User exports a 5-minute recording with 3 cuts and 2 zoom regions

1. `ExportPanel.svelte` `on:click={export}`
2. `invoke('enqueue_export', { req })`
3. `enqueue_export` Tauri command: validates state, enqueues
4. `export_queue` worker thread: 
   a. builds FFmpeg filter graph from `RenderState`
   b. starts ffmpeg with `-f lavfi -i color=...` for background, then
      `-i <source.mp4>`, then the cut/trim `select` filters, then the
      cursor overlay, then annotations, then the zoom-region
      crop+scale, then the output muxer
   c. emits `export:progress` events every ~1% via a channel
5. JS shows progress bar
6. On completion: `output_path` is written to `<output>/exports/`,
   file metadata is inserted into `db` (SQLite), tray "Recent Exports"
   picks it up on next menu rebuild

---

## 11. The contract: preview and export must agree

This is the rule that cuts across the whole architecture. **If you change
any of these, you must change both sides and re-run the parity tests.**

| Concept              | Preview (TS)                          | Export (Rust)                       |
|----------------------|---------------------------------------|--------------------------------------|
| Time math            | `lib/timeline/time-map.ts`            | `render/graph.rs` + `render/mod.rs`  |
| Cut time remap       | `lib/timeline/cuts.ts`                | `render/cursor_export.rs` + `render/graph.rs` |
| Zoom region evaluate  | `video-preview.logic.ts:evaluateZoomAt` | `render/graph.rs:ZoomRegion::scale_at` |
| Cursor interpolate   | `lib/cursor/smoothing.ts`             | `render/cursor_export.rs`            |
| Frame padding        | `lib/editor/frame-padding.ts`         | `render/graph.rs`                    |
| Scene animations     | `lib/scene/animations.ts`             | `render/scene_anim.rs`               |

The Rust export has unit tests (`render::export_parity_test.rs`) that
mirror the JS test fixtures. Run `pnpm test:rust` and `pnpm test:desktop`
together before merging any change that touches either side.

---

## 12. What to read in what order

| If you want to understand...                  | Read these files in order                                                              |
|----------------------------------------------|-----------------------------------------------------------------------------------------|
| Recording pipeline                           | `recording/mod.rs` → `recording/pipeline.rs` → `encoder/h264.rs`                         |
| Editor timeline data model                    | `timeline/cuts.ts` → `timeline/segments.ts` → `timeline/time-map.ts`                       |
| Per-segment speed                            | `timeline/segment-speed.ts`                                                              |
| Hover storyboard (timeline thumbnail)         | `timeline/storyboard.ts` → `timeline/filmstrip.ts` → `timeline/filmstrip-worker.ts`        |
| Playback (the hard one)                      | `playback/clock.ts` → `playback/av-sync.ts` → `packages/media/src/playback/{worker,source}.ts` → `packages/media/src/cache/index.ts` |
| Preview render loop                           | `components/editor/VideoPreview.svelte` → `components/editor/video-preview.logic.ts`     |
| WebGL shader code                            | `components/editor/video-preview.shaders.ts`                                              |
| Export (FFmpeg filter graph)                  | `render/mod.rs` → `render/graph.rs` → `render/cursor_export.rs` → `render/scene_anim.rs` |
| Tauri IPC bridge                              | `commands/editor.rs` (top of the file: command list)                                     |
| Tray / hotkeys / single-instance              | `lib.rs` (run() closure), `tray.rs`, `lib.rs` setup()                                     |
| Project files on disk                         | `project/serialize.rs` + `lib/format/recast.ts` (TS schema)                             |
| Cross-platform support plan                   | `apps/desktop/docs/cross-platform-support-plan.md`                                       |

---

## 13. Known sharp edges (read before changing)

- **Don't introduce new `unwrap()` or `expect()` in code paths reachable
  from a Tauri command.** The Tauri command layer must return `AppResult<T>`;
  any panic there aborts the process (we have no `catch_unwind` in
  `#[tauri::command]`).
- **Don't hold a lock across a `spawn_blocking` call or a `tokio::fs`
  call.** `parking_lot::Mutex` doesn't poison, so it's not a deadlock, but
  it can hold up unrelated threads for the duration of an FFmpeg subprocess
  spawn (up to 100ms). Snapshot the data into a local, drop the guard, then
  do the slow work.
- **Don't add a synchronous (non-`async`) Tauri command that does
  work.** Per `apps/desktop/src-tauri/src/lib.rs:140-150`: any command
  doing CPU/IO/blocking work must be `async + spawn_blocking`. The macOS
  WKWebView freezes for the entire duration of a sync command.
- **The Windows WebView2 has a watchdog that kills processes that block
  the main thread for >5 seconds.** Sync commands, sync FS in a Tauri
  command, or a sync `std::thread::sleep` in `setup()` will trip it.
- **WebCodecs/VideoFrame leaks are silent killers.** A leaked frame
  holds a GPU surface the GC will not reclaim promptly. Every
  `VideoFrame` must have exactly one close path. That path is LRU
  eviction in `packages/media/src/cache/index.ts`
  (`#evictMemoryUntilFits`), plus `clear()` / `evictCache()` /
  `replaceStorage()` for the bulk cases.

  This is not hypothetical: the MediaBunny cache shipped with **no**
  in-memory cap at all — `#memoryInsert` was a bare `Map.set`, nothing
  was ever closed, and `dispose()` deliberately kept the frames. It
  survived three PRs because the perf test asserting the 512 MB cap
  never imported any package code. If you touch the cache, add a test
  that inserts past the cap and asserts frames were closed.
- **The GGML on-device transcription engine (whisper-cpp) SIGILLs on
  CPUs without the SIMD instructions the build machine had.** Always
  build with `TRANSCRIBE_CMAKE_ARGS="-DGGML_NATIVE=OFF"` for portable
  x64. The smoke test in `scripts/release/smoke-test-transcription.ps1`
  catches the failure mode (no badge, exit 1, log `failed to run ffmpeg`).
  See `ci-desktop.yml` for the wiring.
- **The single-instance plugin's OS mutex is keyed on `app.identifier()`.**
  In dev (`tauri dev`) and release (`tauri build`) it's the same string,
  so a developer's running iteration would forward its argv to the
  installed production build. We've `#[cfg]`-gated the plugin off in
  dev builds; don't undo that without a replacement.

---

## 14. How this whole doc stays current

This file is the architecture. It belongs in `apps/desktop/docs/architecture.md`.
When you change a module that the diagram describes:

1. Update the affected `Read these files in order` table.
2. Update the relevant `go look at` pointer at the section.
3. Update any "MUST agree" pairing in §11 if the contract surface moves.
4. If you add a new module that doesn't have a section here, add one.
5. PRs that change architecture without updating this doc are still
   shippable, but reviewers should push back on them.

A good rule: if a new engineer can't build a working mental model of the
codebase from this file alone, the file is wrong. Keep it honest.
