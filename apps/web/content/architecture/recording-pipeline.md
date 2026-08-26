---
kind: architecture
title: "Recording pipeline"
description: "Capture backends, the constant-frame-rate pacer, the encoder, and the cursor, audio and camera tracks."
position: 1
status: production
domain: capture
summary: "Three threads turn a screen selection into the files the editor opens."
inputs:
  - "CaptureTarget: display, window, or region"
  - "System loopback and microphone audio"
  - "OS cursor position sampled at 125 Hz"
outputs:
  - "recording.mp4 (H.264)"
  - "cursor.json"
  - "audio.wav and mic.wav"
  - "camera.mp4"
entrypoints:
  - "apps/desktop/src-tauri/src/recording/mod.rs"
  - "apps/desktop/src-tauri/src/recording/pipeline.rs"
  - "apps/desktop/src-tauri/src/encoder/mod.rs"
  - "apps/desktop/src-tauri/src/capture/"
invariants:
  - "One real second equals one second of video PTS equals one second of cursor time."
  - "The pacer is count-based CFR, and dropped frames are compensated so duration never shifts."
  - "Every FFmpeg spawn goes through configure_silent_command, or Windows flashes a console and steals focus."
  - "A long-lived FFmpeg child's stderr must be drained on a side thread or the pipe fills and stdin deadlocks."
  - "Rust never opens the camera; a preview WebView records it via getUserMedia."
---

## Overview

The recording pipeline is the Rust backend that turns a screen/window/region
selection plus optional audio and camera into a set of on-disk artifacts the
editor later opens. It lives under `apps/desktop/src-tauri/src/` and is driven
from the `start_recording` / `stop_recording` Tauri commands
(`commands/recording.rs`). A single `RecordingManager`
(`recording/mod.rs`) owns at most one live `RecordingSession` at a time and
is stored in `AppState` behind an `Arc` so the blocking start/stop bodies can run
on `spawn_blocking` workers rather than the UI thread.

A session runs three joined threads plus separate OS capture sessions:
a **capture/pacer** thread (`recording/pipeline.rs`), an **encoder** thread
(`encoder/mod.rs`), and a **cursor** sampler thread (`cursor/mod.rs`). The
capture thread pulls raw BGRA frames from a platform `CaptureSource`
(`capture/`), paces them to a constant frame rate, and hands them to the encoder
thread through a bounded `ArrayQueue` (`RecordingPipeline`). The encoder pipes
those frames to a long-lived FFmpeg child for H.264 encoding. Audio (system
loopback + microphone) is captured by independent OS sessions (`audio/`). The
camera is *not* opened by Rust at all; it is recorded in a preview WebView via
`getUserMedia` → `MediaRecorder` and delivered to disk before stop.

The pipeline's central design constraint is **wall-clock ↔ video-PTS ↔
cursor-clock equality**: 1 real second of recording must equal 1 second of video
presentation time and 1 second of cursor-track time, so the editor's stylized
cursor, clicks, zoom triggers, and audio all stay in sync
(`recording/mod.rs`, `recording/pipeline.rs`). This drives the
count-based CFR pacer, the dropped-frame compensation in the encoder, and the
first-frame-offset re-basing of the cursor track.

All timing flows from a single pause-aware `RecordingClock`
 whose `effective_elapsed()` excludes every paused
interval, keeping the video pacer, cursor sampler, and audio writers on one
gap-free timeline across pause/resume.

## Diagram

```mermaid
flowchart TD
    CMD[start_recording command<br/>commands/recording.rs] --> RM[RecordingManager.start<br/>recording/mod.rs]
    RM --> CT[CaptureTarget.resolve<br/>source/crop/scale_factor]
    RM --> CLOCK[RecordingClock<br/>pause-aware]

    CT --> CAP[capture/pacer thread<br/>pipeline.rs]
    CAP --> SRC{CaptureSource<br/>per platform}
    SRC -->|Windows window| WGC[WGC per-window]
    SRC -->|Windows display| DXGI[DXGI duplication]
    SRC -->|macOS| AVF[FFmpeg avfoundation]
    SRC -->|Linux| X11WL[X11 xcb / Wayland portal]

    CAP -->|BGRA frames| Q[RecordingPipeline<br/>ArrayQueue + stats]
    Q --> ENC[encoder thread<br/>encoder/mod.rs]
    ENC -->|rawvideo stdin| FF[FFmpeg H.264 child]
    FF --> MP4[(recording.mp4)]

    CLOCK --> CUR[cursor thread 125Hz<br/>cursor/mod.rs]
    CUR --> CJSON[(cursor.json)]
    CLOCK --> AUD[audio + mic sessions<br/>audio/]
    AUD --> WAV[(audio.wav / mic.wav)]
    PREVIEW[preview WebView<br/>getUserMedia+MediaRecorder] --> CAMFILE[(camera.mp4)]
```

```mermaid
sequenceDiagram
    participant UI as Frontend / Tray / CLI
    participant Cmd as commands/recording.rs
    participant Mgr as RecordingManager
    participant Cap as capture thread
    participant Enc as encoder thread
    participant Cur as cursor thread

    UI->>Cmd: start_recording(target, options)
    Cmd->>Mgr: start() on spawn_blocking worker
    Mgr->>Cap: spawn_capture_loop (pacer)
    Mgr->>Enc: spawn_encoder_loop (FFmpeg)
    Mgr->>Cur: spawn_cursor_capture (125Hz)
    Note over Cap: warmup, first frame sets<br/>first_frame_offset_us
    loop per tick
        Cap->>Enc: push BGRA frame (queue)
        Enc->>Enc: write frame + dup per pacer drop
    end
    UI->>Cmd: stop_recording()
    Cmd->>UI: emit camera-flush (if requested)
    Cmd->>Mgr: stop() waits for camera file
    Mgr->>Cap: stop_flag = true, join
    Mgr->>Enc: drain queue, close stdin, join
    Mgr->>Cur: stop_flag = true, join -> CursorTrack
    Mgr->>Mgr: shift_cursor_track(offset), write cursor.json
    Mgr->>Cmd: RecordingArtifacts
    Cmd->>Cmd: write .recast project + toast warnings
```

## Key components

| Component | File | Responsibility |
|---|---|---|
| `RecordingManager` | `recording/mod.rs` | Owns the single live session; `start`/`stop`/`pause`/`resume`; camera-ready gate; shutdown reaping |
| `RecordingSession` | `recording/mod.rs` | Handles for the 3 threads + audio/mic sessions, clock, paths, camera overlay tracker |
| `RecordingClock` | `recording/mod.rs` | Pause-aware wall clock; `effective_elapsed()` subtracts every paused interval |
| `CaptureTarget` / `resolve*` | `recording/mod.rs` | Resolves display/window/region to `source`+`crop`+`scale_factor` in physical pixels |
| `spawn_capture_loop` | `recording/pipeline.rs` | Count-based CFR pacer; drains `CaptureSource`, emits exactly `fps` frames/sec |
| `RecordingPipeline` | `recording/pipeline.rs` | Bounded `ArrayQueue<VideoFrame>` + captured/dropped/encoded stats |
| `spawn_encoder_loop` | `encoder/mod.rs` | Pipes BGRA rawvideo to FFmpeg H.264; dropped-frame duplication; GOP + quality tiers |
| `pump_stderr_tail` | `encoder/mod.rs` | Drains FFmpeg stderr on a side thread (deadlock avoidance, diagnostics tail) |
| `H264Encoder` / `codec_args` | `encoder/h264.rs` | Per-encoder (NVENC/AMF/QSV/VideoToolbox/libx264) FFmpeg arg generation |
| `CaptureSource` trait | `capture/mod.rs` | Platform-independent full-`source`-sized BGRA frame source; `set_target_fps` hint |
| Windows capture | `capture/platform/windows.rs` | WGC per-window (`WgcSource`), DXGI monitor duplication (`DxgiSource`), xcap fallback |
| macOS capture | `capture/platform/macos.rs` | Long-lived FFmpeg avfoundation child streaming BGRA to a reader thread |
| Linux capture | `capture/platform/linux_x11.rs`, `linux_wayland.rs` | xcb `GetImage` on root; xdg-desktop-portal + PipeWire on Wayland |
| `spawn_cursor_capture` | `cursor/mod.rs` | 125Hz deadline sampler; virtual-desktop→frame mapping; click tracking |
| `sample_cursor_state` | `cursor/platform/` | Win32 `GetCursorPos`/`GetCursorInfo`/`GetAsyncKeyState`; `device_query` on macOS/Linux |
| `shift_cursor_track` | `cursor/mod.rs` | Re-bases whole track earlier by `first_frame_offset_us` so cursor t=0 == video frame 0 |
| `detect_idle_periods` / `detect_zoom_triggers` | `cursor/smoothing.rs` | Post-capture idle windows (2s/5px) and scored auto-zoom candidates |
| Audio sessions | `audio/mod.rs`, `audio/platform/` | WASAPI loopback+mic (Windows); FFmpeg avfoundation/pulse + SCKit (macOS/Linux) |
| `write_camera_track` | `recording/mod.rs` | Normalizes the WebView MediaRecorder blob (MP4/WebM) to plain H.264 MP4, atomic rename |
| `configure_silent_command` | `ffmpeg.rs` | `CREATE_NO_WINDOW` on every FFmpeg/ffprobe spawn (Windows console-flash / focus-steal) |

## Control / data flow

`start_recording` and `stop_recording` in `commands/recording.rs` are thin: they
resolve paths, then hand the real work to `RecordingManager` in
`recording/mod.rs` on a `spawn_blocking` worker. Neither command does anything
heavy on the calling thread, because a sync command blocks the thread WKWebView
paints on.

**Start.** The target is resolved first, and how depends on the platform. On
Wayland the xdg-desktop-portal dialog is negotiated up front and its stream is
stashed for the capture thread, with the portal's dimensions treated as
authoritative. Everywhere else `CaptureTarget::resolve` enumerates monitors and
windows through xcap and computes `source` plus `crop`, then `apply_device_scale`
lifts logical coordinates to physical pixels. That last step is a no-op at scale
1.0 and is the whole reason Retina captures are not half-size.

`RecordingManager::start` then checks screen-recording permission, resolves fps
(`24..=240`, default 60) and the quality tier, and sizes the frame queue from a
256 MB BGRA budget clamped to 30-180 frames. A 4K frame is 33 MB, so that budget
is what stops a slow encoder from turning backpressure into an out-of-memory
kill. It spawns the capture, encoder and cursor threads, starts the system-audio
and microphone sessions, and records that a camera was *requested*; nothing in
Rust opens one. On success the command takes a wake lock and emits
`recording:started`.

**Steady state.** The pacer emits exactly `fps` frames per second, duplicating
the cached last frame when the source produced no new pixels. The encoder writes
each frame to FFmpeg and re-emits one duplicate per pacer-dropped frame, so
`encoded == captured` holds and the file's duration matches the wall clock.
`pause` and `resume` flip a flag and freeze the clock; every producer thread
skips work while it is set, which is why a paused stretch does not appear in the
cursor track either.

**Stop.** If a camera was requested and the preview window is still open,
`stop_recording` emits `camera-flush` and waits up to 30 seconds for the
MediaRecorder bytes to land.

`RecordingManager::stop` then sets the stop flag, joins all three threads, and
stops the audio sessions, **reaping everything before surfacing any error**. The
ordering is the point: returning early on the first failure would orphan an
FFmpeg child or leave a capture device held.

What comes out is assembled in a fixed order. The `CursorTrack` is re-based by
`first_frame_offset_us` through `shift_cursor_track` and written atomically, so
cursor t=0 is video frame 0. System audio resolves to the captured WAV or, if
there was none, a generated silence WAV, because the muxer needs a track either
way. Microphone and camera resolve by presence and push non-fatal warnings when
they are missing.

Finally the command computes duration from encoded frame count divided by fps
rather than from the wall clock, writes the `.recast` bundle, releases the wake
lock, and surfaces any warnings.

## Invariants & gotchas

- **Count-based CFR is the sync backbone.** The encoder declares a fixed
  `-framerate` and feeds timestamp-less rawvideo, so every pushed frame is
  exactly 1/fps of video PTS regardless of capture wall-time. DXGI/WGC only
  deliver on desktop change (a static screen is <1 fps), so the pacer duplicates
  the cached frame to hit the rate; otherwise a 10s low-motion capture would
  encode as 1-2s and race the cursor track (`recording/pipeline.rs`).

- **Dropped-frame compensation preserves duration.** When the queue saturates
  (encoder behind capture), `RecordingPipeline::push` drops the overflow and
  counts it. The encoder re-emits one duplicate of the last frame per drop
  (bounded per iteration, residual flushed after the loop) so `encoded ==
  captured` and the video never plays back sped-up / desynced
  (`encoder/mod.rs`; unit-tested `total_emitted == captured`).

- **Cursor clock re-basing.** The cursor thread ticks from recording start, but
  video frame 0 is whatever the capture-source warmup produced first, i.e.
  video t=0 is wall-clock `first_frame_offset_us`, not 0. Without correction the
  whole cursor track (and clicks/highlights) runs ahead of the video by the
  warmup (~half a second). `stop()` subtracts the recorded offset via
  `shift_cursor_track`, saturating early samples to 0 (`recording/pipeline.rs`,
  `recording/mod.rs`, `cursor/mod.rs`).

- **Virtual-desktop → frame coordinate mapping.** `GetCursorPos` returns
  virtual-desktop coordinates; the video is frame-relative pixels. The cursor
  loop maps `raw * scale - origin`, records `visible=false` for samples outside
  the frame (secondary monitor / cropped region) so the editor hides the cursor
  cleanly rather than clamping it to an edge (`cursor/mod.rs`). The
  `scale` factor lifts macOS logical points into physical pixels (1.0 elsewhere).

- **Deadline scheduling everywhere, computed in integer ns.** Both the pacer
  (`tick_at`, `recording/pipeline.rs`) and the 125Hz cursor sampler
 target absolute tick instants rather than sleeping a
  fixed period, and reset the baseline if they fall >1 period behind (no burst
  catch-up after a stall/pause). The pacer specifically uses `k*1e9/fps` ns to
  avoid the ~0.004%/s drift from truncating `1_000_000/fps` µs.

- **FFmpeg silent-spawn on Windows.** Every FFmpeg/ffprobe `Command` must call
  `configure_silent_command` before spawn, which sets `CREATE_NO_WINDOW`
  (`0x08000000`). Otherwise a console window flashes and steals focus on
  Windows, read by users as the app "freezing" (`ffmpeg.rs`).

- **FFmpeg stderr must be drained continuously.** The encoder's `pump_stderr_tail`
  runs on its own thread for the whole child lifetime. If stderr isn't read, the
  ~64KB OS pipe buffer fills on a long recording, FFmpeg blocks on its stderr
  write, stops reading stdin, and the encoder's `stdin.write_all` deadlocks, freezing capture mid-recording. macOS/Linux hit it sooner (smaller pipe
  buffers). The same reasoning applies to the macOS avfoundation capture child
  (`encoder/mod.rs`, `capture/platform/macos.rs`).

- **`CaptureSource` contract: emit full-`source`-sized frames; the encoder
  crops.** A backend must return `source`-dimensioned BGRA, never pre-cropped: the encoder is configured for `source` dims and applies its own crop filter.
  The X11 backend once pre-cropped, double-cropping and corrupting every
  region/window recording; there are regression tests pinning this
  (`capture/platform/linux_x11.rs`, `recording/mod.rs`).

- **WGC readback throttling.** Windows Graphics Capture delivers a frame per
  window repaint (well above encode rate); each GPU→CPU readback maps GPU memory
  (a GPU stall). `set_target_fps` sets a min-extract interval so surplus frames
  are drained-and-closed cheaply and only one is read back per interval
  (`capture/mod.rs`, `capture/platform/windows.rs`).

- **Camera is never opened by Rust.** Opening the webcam a second time via
  FFmpeg while the preview WebView already holds it fails on single-consumer
  devices. The track is recorded in the preview (`getUserMedia` → `MediaRecorder`),
  delivered via `save_recorded_camera` → `write_camera_track` (which sniffs
  MP4 vs WebM by magic bytes and stream-copies or transcodes to plain H.264 MP4
  with an atomic rename) *before* stop, and gated by the `camera_ready` flag
  (`recording/mod.rs`, `1145-1160`, `1248-1319`).

- **Shutdown reaping.** Quitting from the tray goes through `app.exit(0)` →
  `std::process::exit`, which runs no destructors. `abort_for_shutdown` must be
  called explicitly from the exit handler, and `Drop for RecordingManager` reaps
  a session left live by a panicking owner, otherwise the audio/mic/camera
  children keep running and hold the device (`recording/mod.rs`).

## Related

- [preview-engine.md](/architecture/preview-engine): how the editor
  previews `recording.mp4` and composites the stylized cursor / camera overlay
  from the sidecar tracks this pipeline writes.
- [05-timeline-model.md](/architecture/timeline-model): the render state and cursor /
  zoom-trigger model the recording feeds into.
- [06-export-pipeline.md](/architecture/export-pipeline): where the separate camera
  stream is composited and the final video is re-encoded.
