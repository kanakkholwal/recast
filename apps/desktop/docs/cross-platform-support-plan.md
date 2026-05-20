# Cross-Platform Support Plan — macOS & Linux

> Status: **Planning** · Created 2026-05-19 · Owner: Kanak
>
> Today the app is fully functional **only on Windows**. Recording still
> "works" on macOS/Linux in that a video file is produced, but several
> capture subsystems silently degrade to stubs (silent audio, no cursor
> track, camera errors). Linux screen capture is written but unverified.
> This document inventories what is missing and sequences the work.

---

## 1. Current state by platform

| Subsystem | Windows | Linux | macOS |
|---|---|---|---|
| Screen capture | ✅ DXGI Desktop Duplication | ⚠️ Wayland (portal+PipeWire) & X11 written, **untested** | ❌ xcap fallback only (slow, per-frame reconnect) |
| System audio (loopback) | ✅ WASAPI | ❌ stub — writes silence | ❌ stub — writes silence |
| Microphone | ✅ WASAPI | ❌ stub — writes silence | ❌ stub — writes silence |
| Camera / webcam | ✅ FFmpeg DirectShow | ❌ stub — returns error | ❌ stub — returns error |
| Cursor sampling | ✅ Win32 GetCursorPos | ❌ stub — returns `None` | ❌ stub — returns `None` |
| Audio device list | ✅ WASAPI enumerate | ❌ empty list | ❌ empty list |
| Camera device list | ✅ FFmpeg `-list_devices` | ❌ empty list | ❌ empty list |
| Window capture-exclusion | ✅ `SetWindowDisplayAffinity` | ❌ no-op | ❌ no-op |
| Reveal file in explorer | ✅ `explorer /select` | ❌ no-op | ❌ no-op |
| Video encoding | ✅ FFmpeg (NVENC/x264) | ✅ portable | ✅ portable |
| Delete to trash | ✅ `trash` crate | ✅ portable | ✅ portable |

**Good news:** the architecture already has a clean per-module
`platform/{windows,fallback,...}.rs` abstraction with `#[cfg]` dispatch, so
each gap is an additive, isolated file — no refactor required. FFmpeg is the
codec/format abstraction layer and is already cross-platform.

---

## 2. Phased plan

Ordered by **value-per-effort** — cheapest, highest-confidence wins first.

### Phase 0 — Build & toolchain readiness *(prerequisite)*
- **Per-push compile CI — done 2026-05-19.** `.github/workflows/ci-desktop.yml`
  runs `tauri build --no-bundle` + clippy on Linux/macOS/Windows for every
  push & PR, so cross-platform code stays green while it is written. The
  release workflow (`release-desktop.yml`) already bundles all three on tags.
- WSL note: WSL2 can *compile* the Linux code (needs Rust + the
  `pipewire-upstream` PPA for headers ≥1.4) but **cannot test capture** —
  WSLg ships no `xdg-desktop-portal` ScreenCast backend. CI covers the
  compile gate; functional capture testing needs real Linux/macOS hardware.
- Source per-platform FFmpeg + FFprobe static binaries; place under
  `apps/desktop/binaries/` with the target-triple naming `ffmpeg.rs`
  already expects (`ffmpeg-x86_64-apple-darwin`, `-aarch64-apple-darwin`,
  `-x86_64-unknown-linux-gnu`, etc.). The release workflow already does this.
- Confirm `cargo build` succeeds on each OS with the platform deps
  (`ashpd`, `pipewire`, `x11rb` on Linux). Fix any compile breakage in the
  `fallback.rs` stubs.
- **Exit criteria:** an unsigned dev build launches and records *something*
  on all three OSes.

### Phase 1 — Linux screen capture validation *(low effort, code exists)*
- **Pre-flight static audit — done 2026-05-19. See [Appendix A](#appendix-a)** —
  one critical bug found before any Linux run.
- Get a Linux machine / CI runner (this needs hardware — the `pipewire`,
  `ashpd`, `x11rb` stack links against system C libs and cannot build on
  the Windows dev host).
- Fix audit finding **F1** (missing PipeWire `param_changed` handler) —
  highest-risk, do before first run.
- Test `capture/platform/linux_wayland.rs` on a real Wayland session
  (GNOME + KDE): portal dialog, PipeWire stream, frame pacing.
- Test `capture/platform/linux_x11.rs` on an X11 session.
- Wire the XShm fast path (currently a TODO behind a feature flag) if X11
  GetImage proves too slow.
- **Exit criteria:** Linux screen recording verified at target FPS on both
  session types.

### Phase 2 — Cursor sampling *(low effort, big UX payoff)*
- macOS: `cursor/platform/macos.rs` — `CGEventSource` mouse location +
  `CGEventSourceButtonState` for click state.
- Linux: `cursor/platform/linux.rs` — X11 `XQueryPointer` (via `x11rb`);
  Wayland has no global pointer query, so fall back to capturing pointer
  position from the PipeWire stream metadata or accept no cursor track on
  Wayland (document the limitation).
- Update `cursor/platform/mod.rs` dispatch.
- **Exit criteria:** zoom-trigger / idle detection works on macOS & X11.

### Phase 3 — Camera capture *(medium effort, reuses FFmpeg)*
- macOS: FFmpeg `-f avfoundation` input; enumerate via
  `-f avfoundation -list_devices true`.
- Linux: FFmpeg `-f v4l2` input; enumerate `/dev/video*`.
- Replace `camera/platform/fallback.rs` error stub with `macos.rs` /
  `linux.rs` that mirror `windows.rs` (DirectShow) structure.
- Wire device enumeration in `commands/system.rs`.
- **Exit criteria:** webcam overlay records on all platforms.

### Phase 4 — Audio capture *(high effort, the hard part)*
- macOS system audio: macOS has **no built-in loopback**. Options:
  (a) `ScreenCaptureKit` audio tap (macOS 13+, no extra install — preferred);
  (b) ship/instruct a virtual device (BlackHole). Recommend ScreenCaptureKit.
- macOS microphone: `AVCaptureSession` / CoreAudio `AudioUnit`.
- Linux system audio + mic: PipeWire — capture an audio node the same way
  `linux_wayland.rs` consumes the video stream (the portal can also grant
  audio). ALSA/PulseAudio fallback for non-PipeWire systems.
- Replace `audio/platform/fallback.rs` stubs; wire device enumeration.
- **Exit criteria:** real system audio + mic tracks on macOS & Linux.

### Phase 5 — macOS screen capture *(high effort)*
- Implement `capture/platform/macos.rs` using **ScreenCaptureKit**
  (`SCStream`) — the modern, performant API (macOS 12.3+). Drop the xcap
  fallback for macOS.
- Coordinate with Phase 4: ScreenCaptureKit delivers video **and** audio in
  one stream, so Phases 4 (macOS) and 5 may be done together.
- **Exit criteria:** macOS screen recording at target FPS, no xcap.

### Phase 6 — OS integration polish *(low effort, scattered)*
- Window capture-exclusion: macOS `NSWindow.sharingType = .none`; Linux —
  no portable API, leave no-op + document.
- Reveal-in-explorer: macOS `open -R`; Linux `xdg-open` on parent dir (or
  D-Bus `org.freedesktop.FileManager1.ShowItems` for true select).
- Permissions UX: macOS requires Screen Recording, Microphone, and Camera
  TCC prompts — add first-run permission checks and a settings deep-link
  (`x-apple.systempreferences:`). Linux portal handles its own consent.

### Phase 7 — Packaging, signing & distribution
- macOS: Developer ID signing + **notarization** + stapling for the `.dmg`;
  hardened runtime entitlements for screen/audio/camera.
- Linux: verify AppImage + `.deb`; confirm PipeWire/portal runtime deps are
  declared, not bundled.
- Windows: already configured (MSI/NSIS + timestamp signing).
- Add all three to release CI.

---

## 3. Effort & risk summary

| Phase | Effort | Risk | Notes |
|---|---|---|---|
| 0 Toolchain | M | Low | Pure setup |
| 1 Linux capture validation | S | Low | Code already written |
| 2 Cursor sampling | S | Low | Wayland has a known limitation |
| 3 Camera | M | Low | FFmpeg does the heavy lifting |
| 4 Audio | L | **High** | macOS loopback is the hardest single item |
| 5 macOS screen capture | L | Med | ScreenCaptureKit; pair with Phase 4 |
| 6 OS integration | S | Low | Scattered small items |
| 7 Packaging/signing | M | Med | macOS notarization is fiddly |

**Critical path / biggest unknowns:** macOS system-audio loopback (Phase 4)
and ScreenCaptureKit bring-up (Phase 5). De-risk these early with a spike
before committing the rest of Phase 4–5.

**Suggested milestones:**
- **M1 — Linux beta:** Phases 0, 1, 2 (Linux), 3 (Linux), 4 (Linux).
- **M2 — macOS beta:** Phases 2, 3, 4, 5 (macOS) + 6 + 7.

---

## 4. Key files touched

- `apps/desktop/src-tauri/src/capture/platform/` — `macos.rs` (new),
  validate `linux_wayland.rs` / `linux_x11.rs`
- `apps/desktop/src-tauri/src/audio/platform/` — `macos.rs`, `linux.rs` (new)
- `apps/desktop/src-tauri/src/camera/platform/` — `macos.rs`, `linux.rs` (new)
- `apps/desktop/src-tauri/src/cursor/platform/` — `macos.rs`, `linux.rs` (new)
- `apps/desktop/src-tauri/src/commands/system.rs` — device enumeration,
  window exclusion, reveal-in-explorer per-OS branches
- `apps/desktop/src-tauri/Cargo.toml` — macOS deps
  (`objc2` / `core-graphics` / `screencapturekit` bindings)
- `apps/desktop/src-tauri/tauri.conf.json` — macOS entitlements, signing
- `apps/desktop/binaries/` — per-platform FFmpeg/FFprobe

---

<a name="appendix-a"></a>

## Appendix A — Phase 1 pre-flight audit (2026-05-19)

Static review of the already-written Linux capture code. It cannot be
compiled or run on the Windows dev host, so this is a code-reading pass to
catch bugs before the first Linux run. Findings ranked by first-run risk.

> Note: a prior design doc, `apps/desktop/docs/linux-native-recording.md`,
> was deleted from the working tree during this session. It held the
> original lifecycle diagram and a first-iteration debug list. The relevant
> conclusions are folded into this appendix; recover the file from git
> (`git checkout -- apps/desktop/docs/linux-native-recording.md`) if its
> diagrams are still wanted.

### F1 — CRITICAL · No PipeWire `param_changed` handler
`linux_wayland.rs::pipewire_capture_loop` registers only a `.process()`
listener. It never registers `.param_changed()`, so it never reads the
**actually negotiated** stream format. It assumes:
- the negotiated frame size equals the portal-reported size, and
- the pixel format is BGRA/BGRx.

But `build_format_param` offers `VideoSize` as a **Range (1×1 … 7680×4320)**
and the framerate as a range — the compositor is free to pick a size that
differs from what the portal dialog reported. When it does, every frame
fails the `slice.len() < total` check in `process()` and is silently
dropped → **a recording that is black / zero-length with no error**.
Likewise an RGBA negotiation produces colour-swapped frames.

**Fix (do before first Linux run):** add a `.param_changed()` listener that
parses the `SPA_PARAM_Format` POD into `VideoInfoRaw`, and store the real
`width`/`height`/`format` in shared state the `process()` closure and
`WaylandCaptureSource::{width,height}()` read. This is exactly what the
upstream `pipewire-rs/examples/screencast.rs` does — its omission here is
the single most likely cause of a failed first run. Pin BGRA only (drop the
Range, offer a fixed size) *or* honour whatever comes back; do not assume.

### F2 — HIGH · Encoder configured from portal size, not negotiated size
Same root cause as F1. `WaylandCaptureSource::{width,height}()` return the
portal-reported size; `pipeline.rs` feeds those to the encoder as the input
geometry. If F1's negotiated size differs, the encoder is misconfigured even
if frames were not dropped. Fixed by F1 (read negotiated size).

### F3 — MEDIUM · X11 frame buffer size was unvalidated *(fixed 2026-05-19)*
`linux_x11.rs::capture_next` handed `GetImage`'s reply straight downstream
as BGRA. If the X server packs depth-24 at 24 bpp, or pads scanlines to a
wider `bitmap_pad`, `reply.data.len() != width*height*4` and the encoder
panics or renders striped frames. **Fixed:** added a length check that
returns a clear error naming the geometry mismatch. A real stride-repack
path is still TODO if any tested display actually trips it.

### F4 — MEDIUM (perf) · X11 captures ~4× more than needed
The pacer's drain loop (`pipeline.rs`, `MAX_DRAIN = 4`) calls
`capture_next(0)` up to 4× per tick. `X11CaptureSource` ignores the timeout
and does a full synchronous `GetImage` on every call, returning `Some`
unconditionally — so 3 of every 4 full-screen captures are discarded. At
1080p60 that is ~180 wasted full-frame copies/sec. **Fix:** rate-limit
inside `X11CaptureSource` (record last-capture `Instant`, return `Ok(None)`
if called again within a frame period), or land the XShm fast path.

### F5 — LOW · Portal stream orphaned if `recording_manager.start()` fails
`commands/recording.rs::start_recording` calls `stash_portal_stream()`
*before* `recording_manager.start()`. If `start()` returns `Err`, the
stashed stream (and its open fd) is never consumed. Not a true leak — the
next Wayland recording overwrites the slot via `.replace()` — but the fd
lingers. **Fix:** stash after a successful `start()`, or clear the slot on
the error path.

### F6 — LOW · pipewire version drift in docs *(fixed 2026-05-19)*
Design doc said `pipewire = "0.8"`; Cargo.toml pins `0.9` and the code uses
the 0.9 `Rc` API (`ContextRc`, `MainLoopRc`, `connect_fd_rc`, `StreamBox`).
Doc references corrected before the file was deleted.

### Confirmed-still-present known issues (from the original debug list)
- **Cursor double-render:** `CursorMode::Embedded` burns the compositor
  cursor into frames *and* our own cursor track records positions — the
  export shows two cursors. Switch to `CursorMode::Metadata` once the
  editor's stylized cursor is reliable on Linux.
- **Portal dialog every Record:** `PersistMode::DoNot` saves no consent.
  Switch to `PersistMode::ExplicitlyRevoked` + persist the `restore_token`
  in `AppConfig` for a one-time grant.

### Audit verdict
The architecture is sound and the dispatch logic in `capture/platform/mod.rs`
is correct. **F1 is a genuine bug that will black-screen the first Wayland
run** — fix it before testing. F3 and F6 are fixed. F4 and F5 are
quality/perf items that can follow the first successful capture. None of
this needs re-architecting; Phase 1 remains low-effort once a Linux host is
available.
