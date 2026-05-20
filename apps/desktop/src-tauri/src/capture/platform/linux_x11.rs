//! X11 native screen capture via xcb GetImage.
//!
//! See `apps/desktop/docs/linux-native-recording.md` for the full
//! design. This is the X11 sibling of `linux_wayland`: same
//! `CaptureSource` trait, different transport.
//!
//! ## Why this exists alongside the xcap fallback
//!
//! xcap's X11 path also calls `XGetImage` under the hood, but it spins
//! up a fresh X11 connection on every `capture_image()` call — three
//! orders of magnitude slower than holding the connection open. By
//! owning the connection ourselves we can also escape the per-frame
//! roundtrip latency once we add XShm (TODO below).
//!
//! ## What's not done
//!
//! - **XShm fast path**: enabled in Cargo.toml's feature list, but the
//!   shared-memory handshake (`shmget` → `shm::Attach` → `shm::GetImage`)
//!   isn't wired in yet. Plain `xproto::GetImage` does an XCB roundtrip
//!   per frame which copies pixels through the X protocol; XShm copies
//!   directly into a mapped buffer. ~2-3× speedup at 1080p, more at 4K.
//! - **Window capture** when the target window is partially occluded by
//!   another window — the obscured region returns the front-most
//!   window's pixels instead of the target's content. Fix is to enable
//!   the `composite` feature and call `composite::RedirectWindow` to
//!   get the off-screen pixmap. For MVP we capture the visible portion
//!   only.
//! - **Cursor sprite**: GetImage on the root never includes the cursor.
//!   Our existing `crate::cursor` track records positions; the editor
//!   composites a stylized cursor at playback time. So we don't need
//!   per-frame cursor pixels here. (XFixes can give us the sprite if we
//!   ever need it for "raw cursor" exports — separate follow-up.)
//! - **Pixel format negotiation**: we assume the X server delivers
//!   BGRX/BGRA at 24/32bpp. True for all mainstream X11 desktops. If
//!   we ever see corrupted colors, query the screen visual's
//!   `red/green/blue_mask` to detect byte order and add a swap path.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

use crate::capture::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    let (conn, screen_num) =
        RustConnection::connect(None).context("failed to connect to X11 display")?;
    let setup = conn.setup();
    let screen = setup
        .roots
        .get(screen_num)
        .context("X11 setup did not include the requested screen number")?
        .clone();
    let root = screen.root;

    let depth = screen.root_depth;
    if depth != 24 && depth != 32 {
        return Err(anyhow!(
            "unsupported X11 root depth {depth} — only 24/32bpp tested"
        ));
    }

    // The crop rectangle is in virtual-desktop coordinates (the same
    // space xcap reports). On X11 the root window IS the virtual
    // desktop, so these coordinates pass through directly.
    let x = i16::try_from(target.crop.x)
        .context("X11 capture: crop.x out of i16 range — multi-monitor with extreme offset?")?;
    let y = i16::try_from(target.crop.y).context("X11 capture: crop.y out of i16 range")?;
    let width = u16::try_from(target.crop.width)
        .context("X11 capture: crop.width exceeds u16 — display larger than 65535 px?")?;
    let height =
        u16::try_from(target.crop.height).context("X11 capture: crop.height exceeds u16")?;

    Ok(Box::new(X11CaptureSource {
        conn,
        root,
        x,
        y,
        width,
        height,
    }))
}

struct X11CaptureSource {
    conn: RustConnection,
    root: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

// RustConnection is Send + Sync per x11rb's docs; the explicit unsafe
// impl is unnecessary, but Capture pipeline only requires Send and the
// auto-impl is sufficient.

impl CaptureSource for X11CaptureSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        // ZPixmap returns one row of 4-byte pixels per row at 24/32bpp,
        // padded to a server-defined alignment (`bitmap_pad`). For
        // mainstream X servers (Xorg, Xwayland) on 24/32bpp roots the
        // alignment is always 32 → 4 bytes per pixel exactly, no per-row
        // padding. We trust that here; if pad ever differs we'd see it
        // as a stride mismatch in the encoder.
        //
        // `plane_mask = !0` means "all planes" (full RGB(A)).
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                self.x,
                self.y,
                self.width,
                self.height,
                !0u32,
            )
            .context("X11 GetImage request failed")?
            .reply()
            .context("X11 GetImage reply failed")?;

        // X11 returns BGRX on 24bpp (with the X byte unused) or BGRA on
        // 32bpp visuals. Our encoder treats the pixels as BGRA either
        // way — the alpha byte is irrelevant for screen video, so a
        // garbage X byte is fine.
        //
        // Guard the buffer geometry before handing it downstream: the
        // encoder reads exactly width*height*4 bytes. If the X server
        // packs depth-24 at 24 bits-per-pixel, or pads scanlines to a
        // wider `bitmap_pad`, the reply is a different length and the
        // encoder would either panic or render smeared/striped frames.
        // Fail loudly here instead — the message tells the next
        // iteration exactly what to add a swap/repack path for.
        let expected = self.width as usize * self.height as usize * 4;
        if reply.data.len() != expected {
            return Err(anyhow!(
                "X11 GetImage returned {} bytes, expected {} for {}x{} BGRA — \
                 the X server is not delivering packed 32-bit pixels; a \
                 stride-repack path is needed for this display",
                reply.data.len(),
                expected,
                self.width,
                self.height
            ));
        }
        Ok(Some(reply.data))
    }

    fn width(&self) -> u32 {
        self.width as u32
    }

    fn height(&self) -> u32 {
        self.height as u32
    }
}
