---
kind: post
title: "Bringing Recast to macOS and Linux"
description: "Porting a Windows recorder to three platforms found two bugs that had been in the Windows build the whole time: a synchronous Tauri command that froze WKWebView, and an FFmpeg stderr pipe nobody was draining."
slug: bringing-recast-to-macos-and-linux
date: 2026-06-27
author: Kanak
tags: [engineering, cross-platform, desktop, tauri, rust]
published: true
---

Recast was a Windows app because that was the machine in front of me, not because anyone decided it. Porting it to macOS and Linux turned up two bugs that had been in the Windows build the whole time. Neither is a platform bug. Windows was just hiding both.

## The shape that made the port cheap

Two structural choices, made early for other reasons, did most of the work.

**One module per capability, per platform, behind a trait.** Capture is a folder of files that each implement `CaptureSource` (`capture/mod.rs`):

```rust
pub trait CaptureSource: Send {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn set_target_fps(&mut self, _fps: u32) {}
}
```

Four implementations sit behind it: Windows Graphics Capture per window and DXGI duplication per monitor (`capture/platform/windows.rs`), a long-lived FFmpeg avfoundation child streaming BGRA to a reader thread (`macos.rs`), xcb `GetImage` on the root window (`linux_x11.rs`), and xdg-desktop-portal plus PipeWire (`linux_wayland.rs`). Audio has the same shape: `windows.rs` for WASAPI loopback, `macos_sckit.rs` for ScreenCaptureKit, `ffmpeg_unix.rs` for PulseAudio.

Adding macOS capture meant writing one file that satisfied the trait. It did not mean touching the Windows path, so there was never a day where all three were broken at once.

`set_target_fps` is the one place the abstraction leaks on purpose. WGC delivers a frame on every window repaint rather than on every encoder tick, and each frame costs a GPU→CPU readback that maps GPU memory and stalls the GPU, so it throttles down to the encode rate. DXGI and xcb only produce a frame when the screen actually changes, so they ignore the hint entirely.

**FFmpeg as the shared floor.** Every platform hands BGRA to the same encoder pipeline. WGC, AVFoundation, xcb and PipeWire are four ways to get pixels; below that line there is one code path. The platform-specific surface shrinks to "get me frames", and everything downstream, the pacer, the encoder, the muxer, is written once.

## The freeze that only happened on a Mac

A tester reported the window locking up the moment a recording stopped. Clicks went nowhere until it came back. The same build on Windows was fine.

`stop_recording` was a synchronous `#[tauri::command]`, and inside it the app flushed the encoder, finalized the container, and sometimes re-encoded a camera track. Seconds of real work.

That is invisible on Windows because WebView2 runs the web content in its own process, which keeps painting no matter what the Rust side is doing. macOS runs WKWebView in-process, on the same main thread Tauri dispatches a synchronous command on. Block that thread and the window is frozen for the duration.

The fix is one keyword and a wrapper:

```rust
#[tauri::command]
pub async fn stop_recording(/* … */) -> AppResult<RecordingStopResult> {
    tauri::async_runtime::spawn_blocking(move || { /* the real work */ }).await?
}
```

The interesting part was not the fix. It was that this could not be a single bug, because nothing about `stop_recording` made it special. Walking the rest of the command surface found the same pattern in the device enumerators, the library scan, and reveal-in-folder. Each one was a freeze waiting for a slow disk. The rule is now written down where the commands are registered, and heavy commands are `async` plus `spawn_blocking` without exception, including the startup work that used to run inline in `setup()` and hold the splash window for up to a second.

## The pipe nobody was draining

The second one is a genuine deadlock, and it was in the Windows build too.

The encoder pipes raw BGRA into a long-lived FFmpeg child on stdin. FFmpeg writes its banner and a `frame=… fps=…` progress line to stderr. Nothing was reading stderr.

An OS pipe has a fixed buffer, around 64KB. Once it fills, FFmpeg blocks in `write()` on stderr. A blocked process stops reading stdin. The encoder thread's `stdin.write_all` then blocks forever, and capture freezes mid-recording with no error anywhere.

macOS and Linux default to smaller pipe buffers than Windows, so they hit it sooner. That is the entire difference: the same bug, reached in minutes instead of hours.

`pump_stderr_tail` (`encoder/mod.rs`) drains it on a side thread into a bounded ring, which also means the `child.wait()` on every error path can no longer hang waiting for a stderr-blocked process to exit. Keeping the tail is a side benefit; the drain itself is the fix.

## What the other two platforms actually check for you

A later parity audit across the three backends found the rest of the same class:

- The X11 path applied its crop against the wrong origin on a multi-monitor desktop, because it read the root window rather than the target monitor's offset.
- A failed audio session reported success and produced silence. A recording that is silent for a real reason and one that is silent because capture died looked identical to the app, which is the worst possible failure mode for a recorder.
- FFmpeg children had no `Drop`, so an early return could orphan a process that kept holding the file.

None of these are macOS or Linux bugs. They are ours. Windows was the only platform we ran, so it was also the only platform whose tolerances we had accidentally written to.

## Where each platform stands

Windows is the reference platform: shipped, in use, verified on real hardware.

macOS is code complete and in a tester pass. The honest gaps are hardware verification of the permissions flow, Retina and multi-monitor behaviour, and signing plus notarization. System audio is the awkward one: macOS has no loopback device the way Windows does, so `macos_sckit.rs` goes through ScreenCaptureKit instead of asking the OS for a mirror of the output.

Linux is code complete and waiting on its first hardware pass. Wayland (portal plus PipeWire) and X11 are both written and have been through the audit above, but nothing gets a checkmark from us until it has run on a real GNOME, KDE and X11 session.

Almost none of the cost was writing platform code. It was fixing what two stricter operating systems found in the code we already had.
