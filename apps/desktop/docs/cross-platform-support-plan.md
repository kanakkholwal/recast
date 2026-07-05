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
| **macOS** | ~95% | **~75%** | (a) Out-of-the-box **system audio** needs BlackHole/virtual driver — native ScreenCaptureKit loopback is implemented but **default-off** behind the `sckit-loopback` Cargo feature (upstream apple-metal SDK-symbol issue). (b) Hardware verification of capture + the TCC permission flow + Retina/multi-monitor. (c) Deferred capture-read timeout (a stalled device can hang `stop()`). (d) Developer-ID signing + **notarization**. (e) First-run permissions UX. |
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
| Screen capture | ✅ DXGI Desktop Duplication | 🟢 Wayland (portal+PipeWire) & X11 (XGetImage) | 🟢 FFmpeg AVFoundation |
| System audio (loopback) | ✅ WASAPI | 🟢 FFmpeg pulse `.monitor` (silence fallback if no PA) | ⚠️ BlackHole/virtual driver if installed, else silence + actionable log; native ScreenCaptureKit loopback implemented but **default-off** behind `sckit-loopback` |
| Microphone | ✅ WASAPI | 🟢 FFmpeg pulse `default` | 🟢 FFmpeg avfoundation `:0` |
| Camera / webcam | ✅ FFmpeg DirectShow | 🟢 FFmpeg V4L2 | 🟢 FFmpeg AVFoundation |
| Cursor sampling | ✅ Win32 GetCursorPos | 🟢 device_query (xcb / XWayland) | 🟢 device_query (CoreGraphics) |
| Reveal in file manager | ✅ `explorer /select,` | 🟢 D-Bus `FileManager1.ShowItems` + xdg-open fallback | 🟢 `open -R` |
| Audio device list | ✅ WASAPI enumerate | 🟢 `pactl list short sources` (`.monitor` filtered) | 🟢 AVFoundation listing parsed |
| Camera device list | ✅ FFmpeg `-list_devices` | 🟢 `/dev/video*` + sysfs V4L2 filter | 🟢 AVFoundation listing (screens filtered) |
| Capture capabilities probe | ✅ `capture_capabilities` | 🟢 `capture_capabilities` | 🟢 `capture_capabilities` |
| Window capture-exclusion | ✅ `set_content_protected` (`WDA_EXCLUDEFROMCAPTURE`) | ❌ no-op (no OS API — X11/Wayland have no per-window exclusion) | 🟢 `set_content_protected` (`NSWindow.sharingType`), works vs. AVFoundation capture |
| Video encoding | ✅ FFmpeg (NVENC/x264) | 🟢 FFmpeg (x264, hw if present) | 🟢 FFmpeg (x264, VideoToolbox if present) |
| Delete to trash | ✅ `trash` crate | 🟢 `trash` crate | 🟢 `trash` crate |

The architecture uses a per-module `platform/{windows,macos,linux_*,fallback}.rs`
abstraction with `#[cfg]` dispatch, and FFmpeg as the codec/format layer, so each
gap is an additive, isolated file — no refactor required. **The 🟢 rows are the
whole story: code is written and green in CI on every target; the distance to ✅
is a person sitting in front of a Mac / a Linux box.**

---

## What's left, by area

### macOS — nearest milestone (in active testing)
- **Hardware pass:** capture + TCC permissions + Retina/multi-monitor.
- **System-audio default decision** *(biggest out-of-the-box gap):* enable
  `sckit-loopback` once the upstream apple-metal SDK-symbol issue clears, or
  ship the BlackHole-guided path with clear in-app messaging. `macos_sckit.rs`
  carries a Mac-reviewer smoke-test checklist. When enabled it shares the Screen
  Recording TCC prompt with video, so there's no second permission cost.
- **Deferred capture-read timeout:** interruptible/timeout reads in
  `capture/platform/macos.rs` so a device stall (permission revoked mid-record)
  can't block `stop()`.
- **Permissions UX:** first launch prompts for Screen Recording / Microphone /
  Camera implicitly; capture errors already name the relevant Settings pane. A
  polished first-run flow with deep-links is a follow-up.
- **Phase 5b (deferred):** swap the FFmpeg AVFoundation video source for a
  ScreenCaptureKit `SCStream` video output (lower latency, per-window filtering,
  native HiDPI). The audio half of SCKit is already wired, so the existing TCC
  grant covers it; the cost is objc2 `CMSampleBuffer` → BGRA plumbing. FFmpeg
  AVFoundation is the production bridge until then.

### Linux — one hardware bring-up behind
- **Zero hardware verification** — the biggest unknown. Validate
  `linux_wayland.rs` (portal dialog, PipeWire stream, frame pacing) on GNOME +
  KDE and `linux_x11.rs` on an X11 session.
- **Wayland cursor double-render:** `CursorMode::Embedded` burns the compositor
  cursor into frames *and* we record positions → two cursors in export. Switch
  to `CursorMode::Metadata` once the editor's stylized cursor is reliable on Linux.
- **Portal dialog every record:** `PersistMode::DoNot` saves no consent. Switch
  to `PersistMode::ExplicitlyRevoked` and persist the `restore_token` in
  `AppConfig` for a one-time grant.
- **X11 perf:** XShm fast path is unwired (per-frame XCB roundtrip); the pacer
  drains ~4× per tick while `X11CaptureSource` ignores the timeout and returns a
  fresh full-screen `GetImage` each call (~4× over-capture) — rate-limit inside
  the source or land XShm. Window-occlusion (obscured region returns the
  front-most window's pixels) is not handled.
- **Portal stream lifetime:** `start_recording` stashes the portal stream before
  `recording_manager.start()`; on a `start()` error the stashed fd lingers until
  the next recording overwrites the slot. Stash after a successful start, or
  clear on the error path.

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

- `apps/desktop/src-tauri/src/capture/platform/` — `macos.rs`,
  `linux_wayland.rs`, `linux_x11.rs`, `windows.rs`, `fallback.rs`
- `apps/desktop/src-tauri/src/{audio,camera,cursor}/platform/` — per-OS backends
- `apps/desktop/src-tauri/src/commands/system.rs` — device enumeration, window
  capture-exclusion, reveal-in-file-manager per-OS branches
- `apps/desktop/src-tauri/Cargo.toml` — per-OS deps (`screencapturekit`,
  `device_query`, `ashpd`/`pipewire`/`x11rb`)
- `apps/desktop/binaries/` — per-platform FFmpeg/FFprobe sidecars
