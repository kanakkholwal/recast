# Cross-Platform Support Plan — macOS & Linux

> Status: **Code-complete, verification-gated** · Owner: Kanak · Last updated 2026-07-05
>
> Every capture subsystem — screen, system audio, microphone, camera, cursor,
> device enumeration — is implemented and green in CI on Windows, macOS, and
> Linux. The remaining distance to production is **runtime verification on real
> macOS/Linux hardware**, not missing code. Windows is the shipping reference
> platform; macOS is in active testing; Linux has not yet had a hardware pass.
>
> This doc tracks **current state and what's left**. Implementation history
> (what landed when, hardening passes, fixed audit findings) lives in
> `CHANGELOG.md` — it is deliberately not duplicated here.

---

## Readiness scorecard

"Code complete" = the path is written and compiles in CI for that target.
"Production-ready" = a real user can record → edit → export on that OS without
hitting a known-broken path, verified on hardware.

| OS | Code complete | Production-ready | What's actually left |
|---|---|---|---|
| **Windows** | 100% | **~100% — shipping** | Verified, in users' hands. The reference platform. |
| **macOS** | ~95% | **~75%** | (a) Hardware verification of capture + the TCC permission flow + Retina/multi-monitor. (b) The microphone records the system default only: ScreenCaptureKit names no input devices, so a picked one is ignored. (c) Deferred capture-read timeout (a stalled device can hang `stop()`). (d) Developer-ID signing + **notarization**. (e) First-run permissions UX. |
| **Linux** | ~90% | **~60%** | (a) **Zero hardware verification yet** — biggest unknown. Portal+PipeWire (Wayland) and XGetImage (X11) are written but never run on a real session. (b) Wayland: cursor double-render and a portal dialog on every record (no `restore_token` persistence). (c) X11 perf: XShm fast path unwired, ~4× over-capture, window-occlusion not handled. (d) Functional sign-off on GNOME + KDE + an X11 session. |

**One-line answer:** Windows is done. **macOS is roughly one focused
hardware-test + audio-default + notarization cycle from a public beta.**
**Linux is one hardware bring-up cycle behind macOS** — the code is there, but
nothing has run on a real Linux session, so confidence is lowest.

> Dev-host note: the Linux capture stack (`pipewire`, `ashpd`, `x11rb`) links
> system C libraries and cannot build on the Windows dev host; CI is the compile
> gate, but functional capture testing needs real Linux/macOS hardware.

---

## Current state by subsystem

Legend: ✅ verified on hardware · 🟢 implemented + green in CI, **not yet
hardware-verified** · ⚠️ implemented with a known limitation · ❌ stub/no-op.

| Subsystem | Windows | Linux | macOS |
|---|---|---|---|
| Screen capture | ✅ capturekit (DXGI + Windows Graphics Capture) | 🟢 capturekit (portal+PipeWire / X11) | 🟢 capturekit (ScreenCaptureKit) |
| System audio (loopback) | ✅ capturekit (WASAPI) | 🟢 capturekit (PipeWire sink monitor) | 🟢 capturekit (ScreenCaptureKit tap, no virtual driver) |
| Microphone | ✅ capturekit (WASAPI) | 🟢 capturekit (PipeWire) | ⚠️ capturekit (SCK) — system default only, named inputs unsupported |
| Camera / webcam | ✅ capturekit (Media Foundation) | 🟢 capturekit (V4L2) | 🟢 capturekit (AVFoundation) |
| Cursor sampling | ✅ Win32 GetCursorPos | 🟢 device_query (xcb / XWayland) | 🟢 device_query (CoreGraphics) |
| Reveal in file manager | ✅ `explorer /select,` | 🟢 D-Bus `FileManager1.ShowItems` + xdg-open fallback | 🟢 `open -R` |
| Audio device list | ✅ capturekit (loopback filtered out) | 🟢 capturekit (loopback filtered out) | ⚠️ none — the picker offers "System default" |
| Camera device list | ✅ capturekit | 🟢 capturekit | 🟢 capturekit |
| Capture capabilities probe | ✅ `capture_capabilities` | 🟢 `capture_capabilities` | 🟢 `capture_capabilities` |
| Window capture-exclusion | ✅ `set_content_protected` (`WDA_EXCLUDEFROMCAPTURE`) | ❌ no-op (no OS API — X11/Wayland have no per-window exclusion) | 🟢 `set_content_protected` (`NSWindow.sharingType`), works vs. AVFoundation capture |
| Video encoding | ✅ FFmpeg (NVENC/x264) | 🟢 FFmpeg (x264, hw if present) | 🟢 FFmpeg (x264, VideoToolbox if present) |
| Delete to trash | ✅ `trash` crate | 🟢 `trash` crate | 🟢 `trash` crate |

Screen, camera and audio capture all sit behind `capturekit`, which owns the
per-OS backends; the app holds no `#[cfg]` capture code of its own, and asks
`capturekit::capabilities()` what a platform can do rather than branching on the
target. FFmpeg remains the codec/format layer. **The 🟢 rows are the
whole story: code is written and green in CI on every target; the distance to ✅
is a person sitting in front of a Mac / a Linux box.**

---

## What's left, by area

### macOS — nearest milestone (in active testing)
- **Hardware pass:** capture + TCC permissions + Retina/multi-monitor.
- **Named microphone inputs:** ScreenCaptureKit captures the system default and
  refuses any other device name, so a mic chosen in the picker is ignored (with
  a log line) rather than failing the track. Naming one needs a CoreAudio
  enumeration plus an AVCaptureDevice input backend in capturekit.
- **Deferred capture-read timeout:** a device stall (permission revoked
  mid-record) must not block `stop()`.
- **Permissions UX:** first launch prompts for Screen Recording / Microphone /
  Camera implicitly; capture errors already name the relevant Settings pane. A
  polished first-run flow with deep-links is a follow-up.

### Linux — one hardware bring-up behind
- **Zero hardware verification** — the biggest unknown. Validate capturekit's
  Wayland backend (portal dialog, PipeWire stream, frame pacing) on GNOME + KDE
  and its X11 backend on an X11 session, plus PipeWire audio on both.
- **PipeWire is the only audio backend.** A host running PulseAudio without
  PipeWire loses system audio (silence fallback, reported honestly) and the
  microphone (a warning). Every current mainstream distro ships PipeWire.
- **Wayland cursor double-render:** `CursorMode::Embedded` burns the compositor
  cursor into frames *and* we record positions → two cursors in export. Switch
  to `CursorMode::Metadata` once the editor's stylized cursor is reliable on Linux.
- **Portal dialog every record:** `PersistMode::DoNot` saves no consent. Switch
  to `PersistMode::ExplicitlyRevoked` and persist the `restore_token` in
  `AppConfig` for a one-time grant.
- **X11 perf:** the XShm fast path is unwired, so each read is a full-screen
  `GetImage` over an XCB roundtrip. Window-occlusion (an obscured region returns
  the front-most window's pixels) is not handled. Both live in capturekit.

### HiDPI / fractional-scaling cursor (macOS + Linux)
`device_query` has no cursor-visibility signal, so `visible` is always `true`
(the frame-bounds check still hides the cursor when it leaves the recorded
area). Under fractional scaling the editor's *stylized* cursor can be slightly
offset from the real one; the recording itself is correct because the capture
bakes the OS cursor. True Wayland-native tracking (libei / PipeWire cursor
metadata) is the long-term fix.

<a name="packaging-and-signing"></a>

### Packaging and signing *(release path wired)*
[`release-desktop.yml`](../../../.github/workflows/release-desktop.yml) builds
and bundles MSI/NSIS (Windows), DMG + updater bundle (macOS), and AppImage +
`.deb` (Linux) on every `v*` tag;
[`ci-desktop.yml`](../../../.github/workflows/ci-desktop.yml) keeps each OS
compiling on every push/PR.

**Still missing for a real macOS public ship** (credential/identity tasks, not
code): Developer-ID signing + **notarization** + stapling for the DMG and
updater bundle (today's DMGs are unsigned, requiring an `xattr -dr
com.apple.quarantine` on first launch), and hardened-runtime entitlements for
`device.camera`, `device.microphone`, and screen capture.

---

## Milestones

- **M1 — macOS beta** *(nearer; already in active testing):* hardware pass on
  capture + TCC + Retina/multi-monitor; decide the system-audio default; land
  the deferred capture-read timeout; Developer-ID signing + notarization. Ship
  behind a "macOS preview" label.
- **M2 — Linux beta:** first hardware bring-up on GNOME + KDE (Wayland) and an
  X11 session; fix Wayland cursor double-render + portal-every-record; X11 perf
  (XShm / over-capture) only if a tested display needs it.

---

## Key files

- `crates/capturekit/src/platform/` — every screen, camera and audio backend
- `apps/desktop/src-tauri/src/{capture,audio,camera}/` — the app's `#[cfg]`-free
  adapters over capturekit
- `apps/desktop/src-tauri/src/cursor/platform/` — the last per-OS backend the
  app still owns
- `apps/desktop/src-tauri/src/commands/system.rs` — device enumeration, window
  capture-exclusion, reveal-in-file-manager per-OS branches
- `apps/desktop/src-tauri/Cargo.toml` — per-OS deps (`device_query`, `winreg`)
- `apps/desktop/binaries/` — per-platform FFmpeg/FFprobe sidecars
