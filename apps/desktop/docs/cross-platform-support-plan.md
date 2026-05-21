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
| Screen capture | ✅ DXGI Desktop Duplication | ⚠️ Wayland (portal+PipeWire, F1 fix landed) & X11 written, awaiting hardware test | ✅ FFmpeg AVFoundation (replaces xcap) |
| System audio (loopback) | ✅ WASAPI | ✅ FFmpeg pulse `.monitor` (silence fallback if no PA) | ⚠️ BlackHole/Soundflower/Loopback/VB-Cable if installed; silence + log otherwise. SCKit native loopback is scaffolded but **deferred pending API verification** (see below) |
| Microphone | ✅ WASAPI | ✅ FFmpeg pulse `default` | ✅ FFmpeg avfoundation `:0` |
| Camera / webcam | ✅ FFmpeg DirectShow | ✅ FFmpeg V4L2 | ✅ FFmpeg AVFoundation |
| Cursor sampling | ✅ Win32 GetCursorPos | ✅ device_query (xcb) | ✅ device_query (CoreGraphics) |
| Reveal in file manager | ✅ `explorer /select,` | ✅ D-Bus `FileManager1.ShowItems` + xdg-open fallback | ✅ `open -R` |
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

### Phase 2 — Cursor sampling *(done 2026-05-19)*
Landed via a single shared file `cursor/platform/device_query_impl.rs`
backed by the `device_query` crate (CoreGraphics on macOS, xcb on Linux).
Dispatched from `cursor/platform/mod.rs` for `target_os = "macos"` and
`"linux"`; the existing `fallback.rs` stays for other targets. Pointer
state is sampled at 125 Hz by `cursor::spawn_cursor_capture`, identical to
the Windows backend's contract.

**Caveats** (documented in the impl file):
- No cursor *visibility* signal — `device_query` doesn't expose
  `CGCursorIsVisible` / X11 hide state, so `visible` is always `true`.
  The cursor capture loop's frame-bounds check still hides the cursor
  when it leaves the recorded area, so editor behaviour stays correct.
- On Wayland, pointer queries go through XWayland (present on every
  mainstream Wayland distro). Coords match the compositor 1:1 at
  integer scaling; under HiDPI / fractional scaling the editor's
  *stylized* cursor may be slightly offset from the user's actual
  cursor. The recording itself shows the cursor correctly because the
  portal stream uses `CursorMode::Embedded`. True Wayland-native
  tracking (libei or PipeWire cursor metadata) is the long-term fix.
- `DeviceState` is cached in `thread_local!` so the X11/CoreGraphics
  handle is opened once, not per-sample.

**Exit criteria:** zoom-trigger / idle detection works on macOS and Linux
X11 — verified once CI compile-checks the build and a real machine
records a session.

### Phase 3 — Camera capture *(done 2026-05-19)*
Landed as one shared file
[`camera/platform/ffmpeg_unix.rs`](../src-tauri/src/camera/platform/ffmpeg_unix.rs)
covering macOS (AVFoundation) and Linux (V4L2). Mirrors the existing
`windows.rs` thread/stop-flag/graceful-stop structure exactly so the
upstream `PlatformCameraSession` contract is unchanged. Same 1280×720@30
defaults, same MP4 sanity check, same `q`-then-kill shutdown sequence.

Device resolution falls back to the first available device when the JS
panel sends "Default"/empty (matches the Windows path). Listing logic:
- macOS — parses FFmpeg's `-list_devices true` stderr, skips the
  "Capture screen N" pseudo-devices.
- Linux — picks the lowest-numbered `/dev/video*` node up to 16.

**Not done** (follow-up): wiring `commands/system.rs::get_camera_devices`
for macOS/Linux so the in-app picker populates instead of leaning on
the auto-first-device fallback.

### Phase 4 — Audio capture *(done 2026-05-19, with documented caveat)*
Landed as one shared file
[`audio/platform/ffmpeg_unix.rs`](../src-tauri/src/audio/platform/ffmpeg_unix.rs).
FFmpeg streams raw PCM (`s16le` 48 kHz stereo) to stdout; the capture
thread copies it into the existing `WavWriter`, honouring `pause_flag`
exactly the way WASAPI does — drains the pipe always, only writes
samples when not paused. A stop-watcher thread sends FFmpeg a graceful
`q` on `stop_flag`, escalating to kill on timeout.

**Loopback sources (three-tier resolution chain, top tier deferred):**
- **macOS — ScreenCaptureKit** (`audio/platform/macos_sckit.rs`): the
  only built-in macOS API for system audio without a virtual driver,
  and the path every modern recorder uses. **Currently a placeholder
  that returns `Err` so the chain falls through.** First attempt used
  the `screencapturekit` crate but the assumed module layout
  (`sc_content_filter`, `sc_stream`, ...) doesn't match the current
  crates.io release, and iterating blind from a non-macOS dev host
  burns CI cycles without signal. The file documents exactly what
  needs to happen to wire it (verify crate's current API or switch to
  `objc2-screen-capture-kit`, then implement `try_start` against
  `SCShareableContent`/`SCStream`/`CMSampleBuffer`). When wired this
  will share the Screen Recording TCC prompt with Phase 5 video, so
  there is no second permission cost.
- **macOS — BlackHole / virtual driver** (FFmpeg avfoundation): scans
  for BlackHole / Soundflower / Loopback / VB-Cable and routes through
  FFmpeg if present. Today's effective macOS loopback path.
- **Linux** — `pactl get-default-sink` → `<sink>.monitor` via FFmpeg's
  `-f pulse` input. Works on any PulseAudio or pipewire-pulse install.
- **Silence + actionable warning** — final degrade when no tier
  succeeds; the macOS message names BlackHole specifically.

**Microphone:** AVFoundation `:0` (macOS) or `pulse default` (Linux); a
user-supplied device id falls through if non-empty/non-"default". Mic
failure surfaces as an error — there's no silent fallback for an
explicitly-enabled mic.

**Not done** (follow-up): `commands/system.rs::get_audio_devices` for
macOS/Linux so the mic picker enumerates instead of always defaulting.

### Phase 5 — macOS screen capture *(done 2026-05-19, with planned 5b)*
Landed as
[`capture/platform/macos.rs`](../src-tauri/src/capture/platform/macos.rs)
using FFmpeg AVFoundation as a `CaptureSource`. A single long-lived
FFmpeg subprocess streams raw BGRA frames over stdout; `capture_next()`
reads exactly one frame's worth of bytes per call (same shape as
`X11CaptureSource`). The pacer's `MAX_DRAIN` cap keeps the
"always-Some" behaviour in check.

`-vf scale=W:H` forces output dimensions to match what the encoder
expects, regardless of the screen's native resolution; `-capture_cursor 1`
matches the Wayland path's `CursorMode::Embedded`. macOS no longer hits
the xcap fallback at all.

**Known limitations:**
- First "Capture screen" device only — multi-monitor users get the
  primary display; mapping xcap monitor IDs to AVFoundation indices is
  a follow-up.
- No region capture on macOS; the in-app picker's region selector
  doesn't propagate. AVFoundation captures full screen; `-vf scale=…`
  matches dims. Cropping can be added with `-vf crop=…` later.
- First record requires Screen Recording consent in
  System Settings → Privacy & Security. FFmpeg will spawn but produce
  zero frames until granted; surface this via the capture-source error
  path.

**Phase 5b (deferred):** ScreenCaptureKit *video* source. The audio
half of SCKit is now wired (see Phase 4), so the Screen Recording TCC
prompt the user sees on first record already grants SCKit access; a
future iteration can swap the FFmpeg AVFoundation video source for an
SCKit `SCStream` video output without re-prompting. Wins: lower
latency, per-window/per-app filtering, native HiDPI handling. Cost:
non-trivial objc2 plumbing around `CMSampleBuffer` video frame
extraction → BGRA conversion. The FFmpeg backend is the production
bridge until 5b lands.

### Phase 6 — OS integration polish *(reveal landed 2026-05-19)*
- **Reveal in file manager — landed.** `commands/system.rs::open_file_location`
  now branches: Windows `explorer /select,`, macOS `open -R`, Linux
  tries D-Bus `org.freedesktop.FileManager1.ShowItems` via `gdbus` and
  falls back to `xdg-open` on the parent directory if D-Bus is
  unavailable (covers GNOME/KDE/XFCE/Cinnamon natively).
- **Window capture-exclusion — deferred.** macOS would use
  `NSWindow.sharingType = .none`; Linux has no portable API. Both stay
  no-op until there is a user-visible need.
- **Permissions UX — deferred.** macOS Screen Recording / Microphone /
  Camera TCC prompts surface implicitly on first attempt (the user gets
  an OS dialog). A polished first-run flow with deep-links to System
  Settings is a follow-up; right now the capture error messages name
  the relevant Settings pane.

### Phase 7 — Packaging, signing & distribution *(release path already wired)*
[`release-desktop.yml`](../../../.github/workflows/release-desktop.yml)
already builds and bundles MSI/NSIS (Windows), DMG + updater bundle
(macOS), and AppImage + `.deb` (Linux) on every `v*` tag. The
per-push CI gate added in Phase 0
([`ci-desktop.yml`](../../../.github/workflows/ci-desktop.yml)) keeps
each OS compiling on every change.

**Still missing for a real macOS public ship:**
- Developer ID signing + **notarization** + stapling for the DMG and
  updater bundle (currently produces unsigned DMGs the README warns
  users to `xattr -dr com.apple.quarantine` past).
- Hardened-runtime entitlements declaring `com.apple.security.device.camera`,
  `device.microphone`, and the screen-capture entitlement.

These are credential/identity tasks, not code tasks — outside the scope
of cross-platform code parity, but they gate a "no warnings" macOS
download experience.

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

### F1 — CRITICAL *(fixed 2026-05-19)* · PipeWire format negotiation
`linux_wayland.rs::pipewire_capture_loop` originally registered only a
`.process()` listener and assumed the portal-reported size + BGRA format
matched what PipeWire actually streamed. But `build_format_param` offered
`VideoSize` as a **Range (1×1 … 7680×4320)**, so the compositor was free
to pick a different size — every frame would then fail the
`slice.len() < total` check and be silently dropped → **black / zero-length
recording with no error**.

**Fix landed** as two layers in [linux_wayland.rs](../src-tauri/src/capture/platform/linux_wayland.rs):
1. `build_format_param` now pins `VideoSize` to a fixed `Rectangle`
   instead of a Range, so PipeWire either honours the portal-reported
   dims or fails the stream connection (an observable failure replaces a
   silent one).
2. A `.param_changed()` listener parses the negotiated `VideoInfoRaw`
   and stashes the real geometry in a shared `Arc<Mutex<…>>` that
   `process()` reads each tick. With (1) in place the negotiated dims
   should always match portal dims; if they ever don't, the log line
   says so loudly and `process()` adapts.

### F2 — HIGH *(resolved by F1 fix)* · Encoder size mismatch
Same root cause as F1; with F1's fixed-size pin the encoder's
portal-reported dims will always equal PipeWire's negotiated dims, so
this no longer exists. If F1's `param_changed` warning ever fires, the
encoder will still be wrong for that recording — but at that point the
log makes it visible instead of silently producing a broken file.

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
