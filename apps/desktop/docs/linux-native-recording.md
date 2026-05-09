# Linux Native Recording — Wayland (Phase 1b) + X11 (Phase 2)

> Status: **implementation written, untested on Linux.** Build host is
> Windows. The Rust code under `cfg(target_os = "linux")` has not been
> compiled or run yet — first iteration will happen on a Linux machine.
>
> Both the Wayland (PipeWire portal) and X11 (xcb GetImage) backends are
> implemented; runtime dispatch picks one based on session env vars.

## What's in place

### Wayland (Phase 1b)

- **xdg-desktop-portal ScreenCast** handshake via `ashpd 0.10`.
- **PipeWire stream** consumption via `pipewire 0.9`, on a dedicated
  thread running the libpw main loop. Version pinned to match what
  `xcap` pulls in transitively — see Cargo.toml comment.
- BGRA frame delivery into the existing `CaptureSource` trait, so
  everything downstream of the capture layer (frame pacer, encoder,
  cursor track, audio mux) is unchanged.
- Source-selection bypass: on Wayland sessions the OS portal dialog
  becomes the source picker. Our in-app picker is still shown but its
  selection is overridden by whatever the user picks in the portal
  dialog.

### X11 (Phase 2)

- **xcb GetImage** capture against the root window via `x11rb 0.13`.
- Connection held open across the recording so we don't pay
  setup-latency per frame (xcap reconnected each call — that's why our
  Linux fallback used to feel slow).
- Source picker stays in-app on X11 (no portal involved); existing
  xcap-based monitor/window enumeration provides the source list.
- XShm fast path is **not yet wired** — feature flag is enabled in
  Cargo.toml so the dependency is present but the shared-memory
  handshake is a TODO. ~2–3× speedup pending.

## Build prerequisites on Linux

System packages:

```bash
sudo apt install \
  libpipewire-0.3-dev \
  libdbus-1-dev \
  libxkbcommon-dev \
  pkg-config \
  build-essential
```

`pipewire-rs` builds against the system `libpipewire-0.3.so`, so the dev
headers must be present. The pkg-config name is `libpipewire-0.3`. The
X11 path uses `x11rb` which is pure-Rust (no extra system headers
needed beyond what xcap already requires).

For Ubuntu 22.04, PipeWire 0.3 is the system default since 22.10; check
with `pkg-config --modversion libpipewire-0.3`. Anything 0.3.40+ should
work with the `pipewire = "0.8"` crate.

## File map

| File | Role |
|------|------|
| [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml) | Linux-only `[target.'cfg(target_os = "linux")']` deps: `ashpd`, `pipewire`, `x11rb` |
| [`src-tauri/src/capture/platform/mod.rs`](../src-tauri/src/capture/platform/mod.rs) | Dispatch: Wayland → `linux_wayland`, X11 → `linux_x11`, otherwise → `fallback` |
| [`src-tauri/src/capture/platform/linux_wayland.rs`](../src-tauri/src/capture/platform/linux_wayland.rs) | Portal handshake + PipeWire main loop + `CaptureSource` impl |
| [`src-tauri/src/capture/platform/linux_x11.rs`](../src-tauri/src/capture/platform/linux_x11.rs) | xcb GetImage `CaptureSource` impl |
| [`src-tauri/src/commands/recording.rs`](../src-tauri/src/commands/recording.rs) | Pre-negotiates portal stream before spawning capture threads (Wayland only) |

## Lifecycle

```
[Frontend Record button]
      │
      ▼
start_recording (Tauri sync command)
      │
      ▼ if cfg(linux) && WAYLAND_DISPLAY:
acquire_portal_stream()  ─── blocks while portal dialog is on screen
      │                      (user picks source there)
      ▼
PortalStream { fd, node_id, width, height }
      │
      ▼
stash_portal_stream(stream)         ←── module-static handoff slot
      │
      ▼
RecordingManager::start(target /* with portal-supplied dims */)
      │
      ▼
spawn_capture_loop  ─── new thread
      │
      ▼
create_capture_source(target)
      │
      ▼ take_pending_stream()       ←── retrieves from slot
WaylandCaptureSource::new(stream)
      │
      ▼ spawns recast-pipewire thread
pipewire_capture_loop runs MainLoop
      │
      ▼ on_process callback
ArrayQueue<Vec<u8>>  ←── BGRA frames
      │
      ▼ CaptureSource::capture_next polls the queue
[existing pipeline: pacer → encoder → MP4]
```

## Known issues / first-iteration debug list

These are the things most likely to bite when running this for the first
time on Linux. Keep this list ranked when working through it.

1. **API surface drift in `pipewire 0.8`**. The SPA POD construction in
   `build_format_param` mirrors the upstream `screencast.rs` example —
   minor symbol renames (`ChoiceFlags`, `SpaTypes::ObjectParamFormat`,
   `FormatProperties::*`) are the most common compile failure. Check
   docs.rs/pipewire/0.8 for current names; the structure should not
   need to change.

2. **`Pod::from_bytes` lifetime**. We bind it to a local with an
   intentionally narrow scope. If the borrow checker complains, the fix
   is usually a `let pod = Pod::from_bytes(&bytes)?;` higher up.

3. **`MainLoop::clone()`**. The shutdown timer captures the loop to
   call `quit()` from inside its callback. If `MainLoop` isn't `Clone`
   in this version, switch to `WeakMainLoop` (or move the loop into the
   timer closure with appropriate scoping).

4. **Stride mismatch**. `process` callback handles row-padded buffers
   by copying row-by-row when `stride != width*4`. If frames look
   corrupted (vertical stripes / smearing), log `chunk.stride()` and
   `data.data().len()` to verify the math.

5. **Format negotiation falling back to RGBA**. We declare BGRA/BGRx
   only. If a compositor refuses, the stream errors silently. Add
   `RGBA`/`RGBx` to the alternatives list in `build_format_param` and
   add a channel-swap branch in `process`.

6. **Cursor double-rendering at export**. We request
   `CursorMode::Embedded` so the compositor draws the cursor into the
   frames, but our own cursor track ALSO records positions for
   editor-side stylization. The exported video may show both cursors
   stacked. Resolution: switch to `CursorMode::Metadata` once the
   editor's stylized cursor is reliable on Linux exports — that gets us
   raw cursor sprite + position from the portal without burning it
   into the frames.

7. **Dialog shows on every Record click**. `PersistMode::DoNot` means
   no saved consent. To get a one-time grant, switch to
   `PersistMode::ExplicitlyRevoked` and persist the returned
   `restore_token` in `AppConfig`. Worth doing soon — having the
   dialog pop every time is annoying.

8. **xcap on Wayland still gets called from `Monitor::all` /
   `Window::all` paths in `recording/mod.rs::resolve_*`**. We bypass
   those entirely on Wayland in `commands/recording.rs::start_recording`
   so this should not fire — but if anything else hits xcap on Wayland
   it'll still trigger portal dialogs. Grep for `xcap::` and audit.

## What's deliberately not done

- **XShm fast path on X11** — `x11rb` feature is enabled but the
  shmget→Attach→GetImage flow isn't implemented yet. Plain GetImage is
  fine at 1080p; matters at 4K.
- **XComposite for occluded window capture on X11** — current X11 path
  captures the visible portion of the root window, so a window covered
  by another window will record the front-most pixels in the obscured
  region. Fix: enable `x11rb` `composite` feature, redirect the target
  window to off-screen storage, GetImage from that pixmap.
- **Linux audio capture** — recording audio on Linux still goes through
  whatever the existing `audio/platform/fallback.rs` does. PipeWire
  audio is a separate follow-up; cleanest is to capture an audio node
  via the same portal+PipeWire stack as video.
- **Hardware encode (VAAPI)** — exports still use libx264 software.
  VAAPI integration is a Phase 3 perf task.
- **Saved portal consent** — see issue 7 above.
- **Multi-display capture in one stream** — portal supports it, we
  request `multiple: false` for simplicity.
- **Region capture on Wayland** — the portal doesn't expose sub-rect
  selection. We capture the whole monitor the user picks; the encoder
  could crop in software, but UI for picking the rectangle is on hold
  until real-world Wayland use establishes whether anyone needs it.

## When this lands

After the first Linux compile-and-run cycle resolves any issues from
the list above, fold the lessons back into this doc, then file Phase 2
(X11 native backend) as a follow-up using the same file layout and
trait abstraction.
