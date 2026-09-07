//! What to record, resolved from capturekit's enumeration, so every rectangle is already physical pixels.
//! The old two-way logical conversion was behind half-size Retina captures and regions landing on the wrong monitor.

use anyhow::{anyhow, Context, Result};
use capturekit::{Display, DisplayId, Rect, Window, WindowId};
use serde::{Deserialize, Serialize};

use super::display_at;

/// A rectangle in physical device pixels, in virtual-desktop coordinates: capturekit's own type.
/// The capture stack, the encoder crop and the project file all describe rectangles, and one of them owning a private copy is how the coordinate spaces drifted apart.
pub type CaptureArea = Rect;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureKind {
    Display,
    Window,
    Region,
}

/// Pixel-space rectangle in virtual desktop coordinates, as the region picker
/// sends it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl From<RegionRect> for CaptureArea {
    fn from(rect: RegionRect) -> Self {
        Self::new(rect.x, rect.y, rect.width, rect.height)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTarget {
    pub kind: CaptureKind,
    /// The display or window this names, as capturekit reports it.
    pub id: u64,
    pub label: String,
    /// What the backend delivers: a whole display, or a window's own surface.
    pub source: CaptureArea,
    /// The part of `source` that reaches the file, in the same coordinates.
    pub crop: CaptureArea,
    /// The display being captured, or the one a window sits on. Distinct from
    /// `id`, which is the window itself, and used to re-base the cursor track.
    #[serde(default)]
    pub display_id: u64,
    /// Physical pixels per logical point on `display_id`, which the cursor track
    /// uses to lift its logical samples into the frame's space.
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f32,
}

fn default_scale_factor() -> f32 {
    1.0
}

impl CaptureTarget {
    /// Resolve what the picker named against what the platform reports now.
    pub fn resolve(target_type: &str, target_id: u64) -> Result<Self> {
        let displays = capturekit::displays().context("failed to list displays")?;
        if target_type == "window" {
            let windows = capturekit::windows().context("failed to list windows")?;
            return window_target(
                &windows,
                &displays,
                WindowId(target_id),
                super::window_capture_supported(),
            );
        }
        display_target(&displays, DisplayId(target_id))
    }

    /// Resolve the rectangle the region overlay dragged out.
    pub fn resolve_region(rect: RegionRect) -> Result<Self> {
        let displays = capturekit::displays().context("failed to list displays")?;
        region_target(&displays, rect)
    }

    /// The crop expressed relative to the frame, or `None` when it is the whole
    /// frame and the encoder should not filter at all.
    pub fn crop_relative_to_source(&self) -> Option<CaptureArea> {
        (self.crop != self.source).then(|| self.crop.relative_to(&self.source))
    }

    /// Takes the frame size the opened source actually delivers, which the Wayland portal always differs on.
    /// The crop goes with it, since it described a rectangle of a surface that is no longer being captured.
    pub fn adopt_source_size(&mut self, width: u32, height: u32) {
        if self.source.width == width && self.source.height == height {
            return;
        }
        log::info!(
            "capture source delivers {width}x{height}, not the resolved {}x{}",
            self.source.width,
            self.source.height
        );
        // Rounded down to even, the way `fitted` does: an odd size reaches the encoder as `-video_size 1921x1081`, which libx264 refuses outright and the take is lost.
        let (width, height) = (width & !1, height & !1);
        // The origin survives: the surface did not move, and the cursor uses it.
        self.source = CaptureArea::new(self.source.x, self.source.y, width, height);
        self.crop = self.source;
    }
}

/// The largest even-sized rectangle of `area` that fits inside `bounds`.
/// Even because libx264 and NVENC reject odd dimensions, and clipped because a window can hang off the edge of the display it is on.
fn fitted(area: CaptureArea, bounds: CaptureArea) -> Result<CaptureArea> {
    area.fit_inside(&bounds)
        .context("the capture rectangle does not overlap its display")
}

fn display_by_id(displays: &[Display], id: DisplayId) -> Option<&Display> {
    displays.iter().find(|display| display.id == id)
}

/// A whole display.
pub fn display_target(displays: &[Display], id: DisplayId) -> Result<CaptureTarget> {
    let display = display_by_id(displays, id).context("display target not found")?;
    // The Wayland portal reports no geometry until a surface has been picked.
    let area = if display.bounds.is_empty() {
        display.bounds
    } else {
        fitted(display.bounds, display.bounds)?
    };
    Ok(CaptureTarget {
        kind: CaptureKind::Display,
        id: display.id.0,
        display_id: display.id.0,
        label: display.name.clone(),
        source: area,
        crop: area,
        scale_factor: display.scale_factor,
    })
}

/// One window, as its own surface where the backend can, else as a crop of the display it sits on.
/// `own_surface` is [`super::window_capture_supported`], asked here so the resolved `source` always matches what the backend will deliver.
pub fn window_target(
    windows: &[Window],
    displays: &[Display],
    id: WindowId,
    own_surface: bool,
) -> Result<CaptureTarget> {
    let window = windows
        .iter()
        .find(|candidate| candidate.id == id)
        .context("window target not found")?;
    let display = display_by_id(displays, window.display)
        .or_else(|| display_at(displays, window.bounds.centre()))
        .context("unable to locate the display containing the selected window")?;

    let crop = fitted(window.bounds, display.bounds)?;
    let source = if own_surface {
        crop
    } else {
        fitted(display.bounds, display.bounds)?
    };
    Ok(CaptureTarget {
        kind: CaptureKind::Window,
        id: window.id.0,
        display_id: display.id.0,
        label: window.title.clone(),
        source,
        crop,
        scale_factor: display.scale_factor,
    })
}

/// A rectangle of whichever display holds its centre.
pub fn region_target(displays: &[Display], rect: RegionRect) -> Result<CaptureTarget> {
    let requested = CaptureArea::from(rect);
    if requested.is_empty() {
        return Err(anyhow!("region must have non-zero width and height"));
    }
    let display = display_at(displays, requested.centre())
        .context("unable to locate the display containing the selected region")?;
    let source = fitted(display.bounds, display.bounds)?;
    let crop = fitted(requested, source)?;
    Ok(CaptureTarget {
        kind: CaptureKind::Region,
        id: display.id.0,
        display_id: display.id.0,
        label: format!("Area {}×{}", crop.width, crop.height),
        source,
        crop,
        scale_factor: display.scale_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: u64, bounds: (i32, i32, u32, u32), scale: f32) -> Display {
        Display {
            id: DisplayId(id),
            name: format!("Display {id}"),
            bounds: Rect::new(bounds.0, bounds.1, bounds.2, bounds.3),
            scale_factor: scale,
            refresh_hz: None,
            is_primary: id == 1,
            rotation: capturekit::Rotation::None,
        }
    }

    fn window(id: u64, display: u64, bounds: (i32, i32, u32, u32)) -> Window {
        Window {
            id: WindowId(id),
            display: DisplayId(display),
            title: format!("Window {id}"),
            app_name: "Test".into(),
            pid: 42,
            bounds: Rect::new(bounds.0, bounds.1, bounds.2, bounds.3),
            is_on_screen: true,
            is_minimized: false,
        }
    }

    /// A 2x laptop panel beside an external 1x, which is where the old
    /// logical-versus-physical conversions went wrong.
    fn desktop() -> Vec<Display> {
        vec![
            display(1, (0, 0, 2880, 1800), 2.0),
            display(2, (2880, 0, 1920, 1080), 1.0),
        ]
    }

    fn region(x: i32, y: i32, width: u32, height: u32) -> RegionRect {
        RegionRect {
            x,
            y,
            width,
            height,
        }
    }

    /// Every rectangle capturekit reports is already physical, so a Retina
    /// display resolves at its full pixel size rather than half of it.
    #[test]
    fn a_retina_display_resolves_at_its_physical_size() {
        let target = display_target(&desktop(), DisplayId(1)).expect("the display resolves");
        assert_eq!((target.source.width, target.source.height), (2880, 1800));
        assert_eq!(target.scale_factor, 2.0);
    }

    #[test]
    fn an_unknown_display_is_an_error_rather_than_a_guess() {
        assert!(display_target(&desktop(), DisplayId(99)).is_err());
    }

    /// The whole frame is the crop, so the encoder must not run a crop filter.
    #[test]
    fn a_whole_display_needs_no_crop_filter() {
        let target = display_target(&desktop(), DisplayId(2)).expect("the display resolves");
        assert_eq!(target.crop_relative_to_source(), None);
    }

    #[test]
    fn a_window_on_its_own_surface_is_sized_to_the_window() {
        let windows = [window(7, 2, (2900, 100, 800, 600))];
        let target = window_target(&windows, &desktop(), WindowId(7), true).expect("resolves");
        assert_eq!(target.source, target.crop);
        assert_eq!(target.source.width, 800);
        assert_eq!(target.display_id, 2, "the cursor rebases onto this display");
        assert_eq!(target.crop_relative_to_source(), None);
    }

    /// Without per-window capture the display is the frame and the window is a
    /// rectangle inside it, which is what the encoder's crop filter takes.
    #[test]
    fn a_window_without_its_own_surface_becomes_a_crop_of_its_display() {
        let windows = [window(7, 2, (2900, 100, 800, 600))];
        let target = window_target(&windows, &desktop(), WindowId(7), false).expect("resolves");
        assert_eq!(target.source.width, 1920);
        let crop = target.crop_relative_to_source().expect("a crop");
        assert_eq!(crop, Rect::new(20, 100, 800, 600));
    }

    /// A window half off the right edge would otherwise ask the encoder to crop
    /// past the frame.
    #[test]
    fn a_window_hanging_off_its_display_is_clipped_to_it() {
        let windows = [window(7, 2, (4600, 0, 800, 600))];
        let target = window_target(&windows, &desktop(), WindowId(7), true).expect("resolves");
        assert_eq!(target.crop.width, 200, "4800 - 4600");
    }

    /// A window whose display id is stale still records, because its rectangle
    /// says which display it is on.
    #[test]
    fn a_window_naming_an_unknown_display_falls_back_to_the_one_it_sits_on() {
        let windows = [window(7, 404, (2900, 100, 800, 600))];
        let target = window_target(&windows, &desktop(), WindowId(7), true).expect("resolves");
        assert_eq!(target.display_id, 2);
    }

    #[test]
    fn an_unknown_window_is_an_error() {
        assert!(window_target(&[], &desktop(), WindowId(7), true).is_err());
    }

    /// The region picker sends physical pixels and the displays are physical, so
    /// a region on the Retina panel lands on it rather than on its neighbour.
    #[test]
    fn a_region_lands_on_the_display_that_holds_it() {
        let target = region_target(&desktop(), region(100, 100, 640, 480)).expect("resolves");
        assert_eq!(target.display_id, 1);
        assert_eq!(target.source.width, 2880);
        assert_eq!(
            target.crop_relative_to_source().expect("a crop"),
            Rect::new(100, 100, 640, 480)
        );
    }

    /// A region dragged past the right edge is clipped to what the display has,
    /// or the encoder's crop filter reads outside the frame.
    #[test]
    fn a_region_running_off_the_display_is_clipped_to_it() {
        let target = region_target(&desktop(), region(2500, 0, 640, 480)).expect("resolves");
        assert_eq!(
            target.display_id, 1,
            "its centre is still on the first panel"
        );
        assert_eq!(target.crop.width, 380, "2880 - 2500");
    }

    /// A region straddling two displays belongs to the one holding its centre,
    /// and is clipped to that one. Only one surface can be captured.
    #[test]
    fn a_region_straddling_two_displays_goes_to_the_one_holding_its_centre() {
        let target = region_target(&desktop(), region(2700, 0, 640, 480)).expect("resolves");
        assert_eq!(target.display_id, 2, "the centre sits at x = 3020");
        assert_eq!(target.crop.width, 460, "3340 - 2880");
    }

    #[test]
    fn a_region_with_no_area_is_refused() {
        assert!(region_target(&desktop(), region(0, 0, 0, 100)).is_err());
    }

    #[test]
    fn a_region_entirely_off_screen_is_refused() {
        assert!(region_target(&desktop(), region(-5000, -5000, 640, 480)).is_err());
    }

    /// libx264 refuses odd dimensions, so an odd display is trimmed a pixel
    /// rather than failing at encode time.
    #[test]
    fn odd_dimensions_are_trimmed_to_even() {
        let odd = [display(1, (0, 0, 1921, 1081), 1.0)];
        let target = display_target(&odd, DisplayId(1)).expect("resolves");
        assert_eq!((target.source.width, target.source.height), (1920, 1080));
    }

    /// DXGI reports the raw mode size, untrimmed, so an odd display wrote an odd
    /// size back over the even one `fitted` had resolved, and libx264 then
    /// refused `-video_size 1921x1081` and the take was lost.
    #[test]
    fn an_odd_source_size_is_rounded_down_to_even() {
        let portal = [display(0, (0, 0, 0, 0), 1.0)];
        let mut target = display_target(&portal, DisplayId(0)).expect("the portal resolves");
        target.adopt_source_size(1921, 1081);
        assert_eq!((target.source.width, target.source.height), (1920, 1080));
        assert_eq!((target.crop.width, target.crop.height), (1920, 1080));
    }

    /// The Wayland portal reports a 0x0 placeholder and names the real surface
    /// only after the user has picked one. Refusing it here made every Wayland
    /// display recording fail before the dialog could open.
    #[test]
    fn a_display_with_no_reported_geometry_still_resolves() {
        let portal = [display(0, (0, 0, 0, 0), 1.0)];
        let mut target = display_target(&portal, DisplayId(0)).expect("the portal resolves");
        assert!(target.source.is_empty(), "nothing is known until it opens");
        target.adopt_source_size(1920, 1080);
        assert_eq!(target.source, Rect::new(0, 0, 1920, 1080));
    }

    /// The cursor track re-bases onto `crop`'s origin, so a window whose
    /// delivered size differs from its enumerated one must keep its position or
    /// every click lands offset by where the window sits on screen.
    #[test]
    fn adopting_a_new_size_keeps_the_surface_where_it_was() {
        let windows = [window(7, 2, (2900, 100, 800, 600))];
        let mut target = window_target(&windows, &desktop(), WindowId(7), true).expect("resolves");
        target.adopt_source_size(816, 638);
        assert_eq!((target.crop.x, target.crop.y), (2900, 100));
        assert_eq!((target.crop.width, target.crop.height), (816, 638));
    }

    /// The Wayland portal picks its own surface, so a resolved size the backend
    /// contradicts is the backend's to win.
    #[test]
    fn a_backend_that_delivers_another_size_replaces_the_resolved_one() {
        let mut target = display_target(&desktop(), DisplayId(1)).expect("resolves");
        target.adopt_source_size(1280, 720);
        assert_eq!(target.source.width, 1280);
        assert_eq!(target.crop, target.source, "the crop went with it");
        assert_eq!(target.crop_relative_to_source(), None);
    }

    #[test]
    fn a_backend_that_agrees_leaves_the_crop_alone() {
        let mut target = region_target(&desktop(), region(100, 100, 640, 480)).expect("resolves");
        target.adopt_source_size(2880, 1800);
        assert!(
            target.crop_relative_to_source().is_some(),
            "the crop survived"
        );
    }
}
