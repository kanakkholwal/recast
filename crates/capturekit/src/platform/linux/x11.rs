use core::time::Duration;

use capturekit_core::{
    CaptureError, ColorSpace, CursorSample, CursorShape, CursorShapeKind, DirtyRects, Display,
    DisplayId, LostReason, PixelFormat, Rect, Result, Rotation, SourceDesc, Timestamp, Window,
    WindowId,
};
use x11rb::connection::Connection;
use x11rb::protocol::randr::{self, ConnectionExt as RandrExt};
use x11rb::protocol::xfixes::ConnectionExt as XfixesExt;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, ImageFormat, Screen, Window as XWindow};
use x11rb::rust_connection::RustConnection;

use crate::backend::{FrameSource, RawFrame};
use crate::platform::linux::now;
use crate::platform::OpenOptions;

pub(crate) const BACKEND: &str = "x11";

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CaptureError {
    CaptureError::backend(BACKEND, error)
}

/// XFixes 2.0 is where `GetCursorImage` arrives, and every server that has the
/// extension at all has at least that.
const XFIXES_MAJOR: u32 = 2;
const XFIXES_MINOR: u32 = 0;

/// A connection plus the screen it was opened against.
struct Session {
    conn: RustConnection,
    screen: Screen,
}

impl Session {
    fn open() -> Result<Self> {
        let (conn, index) = x11rb::connect(None).map_err(failed)?;
        let screen = conn
            .setup()
            .roots
            .get(index)
            .cloned()
            .ok_or(CaptureError::NotFound {
                kind: "screen",
                id: index as u64,
            })?;
        Ok(Self { conn, screen })
    }

    /// Turn on XFixes, so the cursor can be sampled with the frames.
    ///
    /// The extension has to be negotiated before any of its requests are sent.
    /// Absent on nothing modern, but a server without it just means no cursor.
    fn enable_xfixes(&self) -> bool {
        self.conn
            .xfixes_query_version(XFIXES_MAJOR, XFIXES_MINOR)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some()
    }

    fn atom(&self, name: &[u8]) -> Result<u32> {
        Ok(self
            .conn
            .intern_atom(false, name)
            .map_err(failed)?
            .reply()
            .map_err(failed)?
            .atom)
    }
}

/// Monitors as RandR reports them, which is the only view that matches what a
/// user sees: the X screen is one big drawable spanning every output.
pub(crate) fn displays() -> Result<Vec<Display>> {
    let session = Session::open()?;
    let monitors = session
        .conn
        .randr_get_monitors(session.screen.root, true)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;

    monitors
        .monitors
        .into_iter()
        .map(|monitor| {
            let name = session
                .conn
                .get_atom_name(monitor.name)
                .map_err(failed)?
                .reply()
                .map_err(failed)?;
            Ok(Display {
                // RandR monitor names are unique per screen, so their atom is a
                // stable id; the monitor list index is not, as it renumbers when
                // an output is unplugged.
                id: DisplayId(u64::from(monitor.name)),
                name: String::from_utf8_lossy(&name.name).into_owned(),
                bounds: Rect::new(
                    i32::from(monitor.x),
                    i32::from(monitor.y),
                    u32::from(monitor.width),
                    u32::from(monitor.height),
                ),
                // X11 has no per-monitor scale: toolkits derive one from DPI, and
                // the pixels here are always physical.
                scale_factor: 1.0,
                refresh_hz: refresh_of(&session, &monitor),
                is_primary: monitor.primary,
                rotation: Rotation::None,
            })
        })
        .collect()
}

/// Refresh rate of a monitor's first active CRTC.
fn refresh_of(session: &Session, monitor: &randr::MonitorInfo) -> Option<f32> {
    let output = monitor.outputs.first().copied()?;
    let info = session
        .conn
        .randr_get_output_info(output, 0)
        .ok()?
        .reply()
        .ok()?;
    let crtc = session
        .conn
        .randr_get_crtc_info(info.crtc, 0)
        .ok()?
        .reply()
        .ok()?;
    let resources = session
        .conn
        .randr_get_screen_resources_current(session.screen.root)
        .ok()?
        .reply()
        .ok()?;
    let mode = resources.modes.iter().find(|mode| mode.id == crtc.mode)?;
    let dots = u32::from(mode.htotal) * u32::from(mode.vtotal);
    (dots > 0).then(|| mode.dot_clock as f32 / dots as f32)
}

fn text_property(session: &Session, window: XWindow, property: u32, kind: u32) -> Option<String> {
    let reply = session
        .conn
        .get_property(false, window, property, kind, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    (!reply.value.is_empty()).then(|| String::from_utf8_lossy(&reply.value).into_owned())
}

/// A single 32-bit CARDINAL property, which is how EWMH publishes a pid.
fn cardinal(session: &Session, window: XWindow, property: u32) -> Option<u32> {
    session
        .conn
        .get_property(false, window, property, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
}

pub(crate) fn windows() -> Result<Vec<Window>> {
    let session = Session::open()?;
    let displays = displays()?;
    let client_list = session.atom(b"_NET_CLIENT_LIST")?;
    let net_name = session.atom(b"_NET_WM_NAME")?;
    let utf8 = session.atom(b"UTF8_STRING")?;
    let net_state = session.atom(b"_NET_WM_STATE")?;
    let hidden = session.atom(b"_NET_WM_STATE_HIDDEN")?;
    let net_pid = session.atom(b"_NET_WM_PID")?;

    let listed = session
        .conn
        .get_property(
            false,
            session.screen.root,
            client_list,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    // A window manager that publishes no client list is not one this can
    // enumerate; reporting nothing beats guessing from the whole window tree.
    let Some(handles) = listed.value32() else {
        return Ok(Vec::new());
    };

    let mut windows = Vec::new();
    for handle in handles {
        let Ok(Ok(geometry)) = session.conn.get_geometry(handle).map(|c| c.reply()) else {
            continue;
        };
        // Geometry is relative to the parent the window manager reparented it
        // into, so it has to be translated to get desktop coordinates.
        let Ok(Ok(origin)) = session
            .conn
            .translate_coordinates(handle, session.screen.root, 0, 0)
            .map(|c| c.reply())
        else {
            continue;
        };

        let title = text_property(&session, handle, net_name, utf8)
            .or_else(|| {
                text_property(
                    &session,
                    handle,
                    AtomEnum::WM_NAME.into(),
                    AtomEnum::STRING.into(),
                )
            })
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let app_name = text_property(
            &session,
            handle,
            AtomEnum::WM_CLASS.into(),
            AtomEnum::STRING.into(),
        )
        .map(|class| {
            // WM_CLASS is two NUL-separated strings: instance then class.
            class
                .split('\0')
                .rfind(|part| !part.is_empty())
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default();

        let is_minimized = session
            .conn
            .get_property(false, handle, net_state, AtomEnum::ATOM, 0, 32)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(|mut v| v.any(|atom| atom == hidden)))
            .unwrap_or(false);

        let bounds = Rect::new(
            i32::from(origin.dst_x),
            i32::from(origin.dst_y),
            u32::from(geometry.width),
            u32::from(geometry.height),
        );
        windows.push(Window {
            id: WindowId(u64::from(handle)),
            title,
            app_name,
            pid: cardinal(&session, handle, net_pid).unwrap_or(0),
            display: display_for(&bounds, &displays),
            bounds,
            is_minimized,
            is_on_screen: !is_minimized,
        });
    }
    Ok(windows)
}

fn display_for(bounds: &Rect, displays: &[Display]) -> DisplayId {
    let centre = Rect::new(
        bounds.x + (bounds.width / 2) as i32,
        bounds.y + (bounds.height / 2) as i32,
        1,
        1,
    );
    displays
        .iter()
        .find(|display| display.bounds.intersect(&centre).is_some())
        .or_else(|| displays.iter().find(|display| display.is_primary))
        .map(|display| display.id)
        .unwrap_or(DisplayId(0))
}

/// Display, region and window capture through `GetImage`.
///
/// The x, y, width and height handed to `GetImage` are a **server-side** crop:
/// the X server copies only that rectangle, so a region capture never moves the
/// rest of the screen across the socket. That is this backend's equivalent of the
/// GPU crop the other two do.
///
/// No shared memory yet. `MIT-SHM` would avoid the socket round trip entirely and
/// is the next thing to do here; the `shm` feature is already enabled for it.
pub(crate) struct X11Source {
    session: Session,
    drawable: XWindow,
    /// The rectangle to ask the server for, in the drawable's own coordinates.
    grab: Rect,
    /// Set only when the caller asked for a crop, which is what tells the shared
    /// code above not to crop again on the host.
    region: Option<Rect>,
    desc: SourceDesc,
    frame: Vec<u8>,
    /// Cursor sampling through XFixes, off when the server has no such extension.
    cursor: Option<Cursor>,
}

/// The cursor as XFixes last reported it.
///
/// The image comes back with every call, so the serial is what says whether it
/// actually changed and a consumer can keep its decoded copy.
#[derive(Default)]
struct Cursor {
    shape: Option<CursorShape>,
    serial: u32,
}

/// Where the cursor IMAGE starts, given the position XFixes reports.
///
/// XFixes answers with the hotspot's position on the screen, while every
/// backend here reports the image's top-left, which is what Windows' Desktop
/// Duplication gives. Reporting the hotspot instead offsets the drawn cursor by
/// however far into its own image the point is, which for a standard arrow is a
/// few pixels and for a crosshair is half its width.
const fn image_origin(hotspot: (i32, i32), hot: (u32, u32), surface: &Rect) -> (i32, i32) {
    (
        hotspot.0 - hot.0 as i32 - surface.x,
        hotspot.1 - hot.1 as i32 - surface.y,
    )
}

impl X11Source {
    /// Read the cursor, keeping its image only when the serial says it changed.
    ///
    /// `None` when the server has no XFixes, and `visible: false` when the
    /// pointer is on another screen or hidden, which XFixes reports as a
    /// zero-sized image rather than as an error.
    fn sample_cursor(&mut self, pts: Timestamp) -> Option<CursorSample> {
        let cursor = self.cursor.as_mut()?;
        let image = self
            .session
            .conn
            .xfixes_get_cursor_image()
            .ok()?
            .reply()
            .ok()?;

        if image.width == 0 || image.height == 0 {
            return Some(CursorSample::absent(pts));
        }
        if image.cursor_serial != cursor.serial || cursor.shape.is_none() {
            cursor.serial = image.cursor_serial;
            cursor.shape = Some(CursorShape {
                width: u32::from(image.width),
                height: u32::from(image.height),
                stride: u32::from(image.width) * 4,
                hotspot_x: u32::from(image.xhot),
                hotspot_y: u32::from(image.yhot),
                // XFixes stores ARGB premultiplied, one pixel per 32-bit word.
                kind: CursorShapeKind::PremultipliedColor,
                bytes: image
                    .cursor_image
                    .iter()
                    .flat_map(|pixel| pixel.to_le_bytes())
                    .collect(),
            });
        }
        let (x, y) = image_origin(
            (i32::from(image.x), i32::from(image.y)),
            (u32::from(image.xhot), u32::from(image.yhot)),
            &self.grab,
        );
        Some(CursorSample {
            pts,
            position: Some((x, y)),
            visible: true,
            shape_id: u64::from(image.cursor_serial),
        })
    }

    pub(crate) fn open_display(display: DisplayId, opts: &OpenOptions) -> Result<Self> {
        let session = Session::open()?;
        let monitor = displays()?
            .into_iter()
            .find(|candidate| candidate.id == display)
            .ok_or(CaptureError::NotFound {
                kind: "display",
                id: display.0,
            })?;
        // The root window spans every monitor, so the monitor's own position in
        // the screen is where its pixels start.
        let surface = monitor.bounds;
        let grab = match opts.region {
            Some(region) => region.offset_by(&surface).fit_inside(&surface).ok_or(
                CaptureError::Unsupported {
                    backend: BACKEND,
                    operation: "crop to a region outside the display",
                },
            )?,
            None => surface,
        };
        let root = session.screen.root;
        Self::build(
            session,
            root,
            grab,
            opts.region.map(|_| grab),
            Rotation::None,
            opts,
        )
    }

    pub(crate) fn open_window(window: WindowId, opts: &OpenOptions) -> Result<Self> {
        let session = Session::open()?;
        let handle = window.0 as XWindow;
        let geometry = session
            .conn
            .get_geometry(handle)
            .map_err(failed)?
            .reply()
            .map_err(|_| CaptureError::NotFound {
                kind: "window",
                id: window.0,
            })?;
        let surface = Rect::from_size(u32::from(geometry.width), u32::from(geometry.height));
        let grab = match opts.region {
            Some(region) => region
                .fit_inside(&surface)
                .ok_or(CaptureError::Unsupported {
                    backend: BACKEND,
                    operation: "crop to a region outside the window",
                })?,
            None => surface,
        };
        Self::build(
            session,
            handle,
            grab,
            opts.region.map(|_| grab),
            Rotation::None,
            opts,
        )
    }

    fn build(
        session: Session,
        drawable: XWindow,
        grab: Rect,
        region: Option<Rect>,
        rotation: Rotation,
        opts: &OpenOptions,
    ) -> Result<Self> {
        // Negotiated once at open: a per-frame check would cost a round trip
        // for an answer that cannot change while the connection lives.
        let cursor = session.enable_xfixes().then(Cursor::default);
        Ok(Self {
            session,
            drawable,
            grab,
            region,
            desc: SourceDesc {
                width: grab.width,
                height: grab.height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation,
                scale_factor: 1.0,
                frame_rate: opts.frame_rate(),
                backend: BACKEND,
            },
            frame: Vec::new(),
            cursor,
        })
    }
}

impl FrameSource for X11Source {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn cursor_shape(&self) -> Option<&CursorShape> {
        self.cursor.as_ref()?.shape.as_ref()
    }

    fn next_frame(&mut self, _timeout: Duration) -> Result<RawFrame<'_>> {
        // GetImage is a synchronous grab with no timestamp of its own, so the
        // moment of the call is the only honest answer.
        let pts = now();
        let reply = self
            .session
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.drawable,
                self.grab.x as i16,
                self.grab.y as i16,
                self.grab.width as u16,
                self.grab.height as u16,
                !0,
            )
            .map_err(failed)?
            .reply()
            // A window that closed, or a display that was unplugged, both surface
            // here as a request against a drawable the server no longer has.
            .map_err(|_| CaptureError::Lost(LostReason::WindowClosed))?;

        if reply.depth != 24 && reply.depth != 32 {
            return Err(CaptureError::Unsupported {
                backend: BACKEND,
                operation: "read a visual that is not 24 or 32 bits deep",
            });
        }
        // Sampled next to the grab rather than on a poller of its own, so the
        // cursor and the pixels it sits on share one instant.
        let sample = self.sample_cursor(pts);
        self.frame = reply.data;
        // Z_PIXMAP pads each scanline to the server's bitmap unit, which is 32
        // bits on every modern server, so a 32-bit-per-pixel row is already whole.
        let stride = self.grab.width * 4;
        capturekit_core::PixelFormat::Bgra8.validate_buffer(
            self.grab.width,
            self.grab.height,
            stride,
            self.frame.len(),
        )?;

        Ok(RawFrame {
            pts,
            bytes: &self.frame,
            stride,
            dirty: DirtyRects::unknown(),
            cursor: sample,
        })
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// XFixes answers with the HOTSPOT's position while every backend here
    /// reports the image's top-left. Skipping the correction offsets the drawn
    /// cursor by however far into its own image the point is.
    #[test]
    fn the_reported_position_is_the_images_corner_not_the_hotspot() {
        let screen = Rect::from_size(1920, 1080);
        assert_eq!(image_origin((100, 200), (7, 9), &screen), (93, 191));
    }

    /// A capture of a monitor that does not start at the screen origin, or of a
    /// cropped region, must report the cursor in the captured surface's own
    /// pixels rather than in screen coordinates.
    #[test]
    fn the_position_is_relative_to_the_captured_surface() {
        let second_monitor = Rect::new(1920, 0, 1920, 1080);
        assert_eq!(image_origin((2000, 50), (0, 0), &second_monitor), (80, 50));
    }

    #[test]
    fn a_cursor_left_of_the_captured_surface_reports_a_negative_position() {
        let region = Rect::new(100, 100, 200, 200);
        assert_eq!(image_origin((40, 40), (0, 0), &region), (-60, -60));
    }
}
