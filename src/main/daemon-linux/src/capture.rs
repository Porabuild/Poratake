use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow};
use image::{DynamicImage, RgbaImage};
use poratake_daemon_common::geometry::{
    CaptureAreaRequest, CaptureRect, CaptureWindowRequest, DisplayInfo, DisplayOrigin,
    WindowBounds, WindowInfo,
};
use xcb::{
    Xid, XidNew, composite,
    randr::{GetCrtcInfo, GetOutputInfo, GetOutputPrimary, GetScreenResources},
    x,
};
#[cfg(feature = "wayland")]
use zed_scap::capturer::{Capturer, Options, Resolution};
#[cfg(feature = "wayland")]
use zed_scap::frame::{Frame, FrameType};

use crate::Backend;

const MIN_WINDOW_SIZE: u16 = 50;

#[derive(Clone)]
pub struct FrozenFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: SharedPixels,
    pub origin: DisplayOrigin,
}

#[derive(Clone)]
pub struct SharedPixels(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[derive(Clone, Default)]
pub struct FrozenFrames(Arc<Mutex<Vec<FrozenFrame>>>);

impl FrozenFrames {
    pub fn replace(&self, frames: Vec<FrozenFrame>) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = frames;
    }

    pub fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn crop(&self, rect: CaptureRect) -> Option<RgbaImage> {
        let frames = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        frames.iter().find_map(|frame| {
            contains_region(
                frame.origin,
                (frame.width as i32, frame.height as i32),
                rect,
            )
            .then(|| crop_frozen_frame(frame, rect))
            .flatten()
        })
    }
}

pub(crate) fn x11_connection() -> Result<(xcb::Connection, usize)> {
    let (connection, screen_number) =
        xcb::Connection::connect_with_extensions(None, &[xcb::Extension::RandR], &[])
            .context("could not connect to the X11 display")?;
    Ok((connection, screen_number as usize))
}

/// The screen a connection was opened on, for root-window operations.
pub(crate) fn x11_screen(connection: &xcb::Connection, screen_number: usize) -> Result<&x::Screen> {
    connection
        .get_setup()
        .roots()
        .nth(screen_number)
        .ok_or_else(|| anyhow!("X11 did not expose a screen"))
}

pub fn list_displays(backend: Backend) -> Result<Vec<DisplayInfo>> {
    if backend != Backend::X11 {
        return Err(anyhow!(
            "display discovery is only available in X11 sessions"
        ));
    }
    let (connection, screen_number) = x11_connection()?;
    let screen = x11_screen(&connection, screen_number)?;
    let resources = connection
        .wait_for_reply(connection.send_request(&GetScreenResources {
            window: screen.root(),
        }))
        .context("could not list X11 displays")?;
    let primary = connection
        .wait_for_reply(connection.send_request(&GetOutputPrimary {
            window: screen.root(),
        }))
        .context("could not identify the primary X11 display")?
        .output();
    let mut displays = Vec::new();
    for output in resources.outputs() {
        let info = connection
            .wait_for_reply(connection.send_request(&GetOutputInfo {
                output: *output,
                config_timestamp: resources.config_timestamp(),
            }))
            .context("could not read an X11 display")?;
        if info.connection() != xcb::randr::Connection::Connected || info.crtc().resource_id() == 0
        {
            continue;
        }
        let crtc = connection
            .wait_for_reply(connection.send_request(&GetCrtcInfo {
                crtc: info.crtc(),
                config_timestamp: resources.config_timestamp(),
            }))
            .context("could not read X11 display geometry")?;
        displays.push(DisplayInfo {
            rect: CaptureRect {
                x: crtc.x() as i32,
                y: crtc.y() as i32,
                width: crtc.width() as i32,
                height: crtc.height() as i32,
            },
            primary: *output == primary,
        });
    }
    if displays.is_empty() {
        return Err(anyhow!("X11 did not expose a connected display"));
    }
    Ok(displays)
}

pub fn capture_area_to_file(
    backend: Backend,
    capture: &CaptureAreaRequest,
    frozen: &FrozenFrames,
) -> Result<()> {
    let rect = capture.capture.rect;
    if !rect.has_positive_size() {
        return Err(anyhow!("capture area must have positive dimensions"));
    }

    if capture.cached
        && let Some(image) = frozen.crop(rect)
    {
        return save_png(image, &capture.path);
    }

    let (image, origin) = match backend {
        Backend::Wayland => (capture_wayland_frame()?, capture.capture.display_origin()),
        Backend::X11 => capture_x11_frame(rect)?,
        Backend::Headless => {
            return Err(anyhow!("Linux capture needs an X11 or Wayland session"));
        }
    };
    let cropped = crop_on_display(&image, rect, origin)?;

    save_png(cropped, &capture.path)
}

pub fn capture_area_pixels(backend: Backend, rect: CaptureRect) -> Result<RgbaImage> {
    if backend != Backend::X11 {
        return Err(anyhow!("scroll capture is only available in X11 sessions"));
    }
    if !rect.has_positive_size() {
        return Err(anyhow!("capture area must have positive dimensions"));
    }
    let (image, origin) = capture_x11_frame(rect)?;
    crop_on_display(&image, rect, origin)
}

pub fn capture_display_frames(backend: Backend) -> Result<Vec<FrozenFrame>> {
    if backend != Backend::X11 {
        return Err(anyhow!("freeze screen is only available in X11 sessions"));
    }
    list_displays(backend)?
        .into_iter()
        .map(|display| {
            let (image, origin) = capture_x11_frame(display.rect)?;
            let width = image.width();
            let height = image.height();
            Ok(FrozenFrame {
                width,
                height,
                pixels: SharedPixels(Arc::new(image.into_raw())),
                origin,
            })
        })
        .collect()
}

pub struct X11FreezeOverlay {
    connection: xcb::Connection,
    windows: Vec<x::Window>,
}

impl Drop for X11FreezeOverlay {
    fn drop(&mut self) {
        for window in self.windows.drain(..) {
            self.connection.send_request(&x::DestroyWindow { window });
        }
        let _ = self.connection.flush();
        let _ = self
            .connection
            .wait_for_reply(self.connection.send_request(&x::GetInputFocus {}));
    }
}

pub fn present_frozen_frames(backend: Backend, frames: &[FrozenFrame]) -> Result<X11FreezeOverlay> {
    if backend != Backend::X11 {
        return Err(anyhow!("freeze screen is only available in X11 sessions"));
    }
    let (connection, screen_number) = x11_connection()?;
    let windows = {
        let screen = x11_screen(&connection, screen_number)?;
        let format = x11_pixel_format(
            &connection,
            screen,
            screen.root_depth(),
            screen.root_visual(),
        )?;
        let mut windows = Vec::with_capacity(frames.len());
        for frame in frames {
            let width = u16::try_from(frame.width).context("freeze frame was too wide for X11")?;
            let height =
                u16::try_from(frame.height).context("freeze frame was too tall for X11")?;
            let x = i16::try_from(frame.origin.x).context("freeze frame x was outside X11")?;
            let y = i16::try_from(frame.origin.y).context("freeze frame y was outside X11")?;
            let pixmap: x::Pixmap = connection.generate_id();
            let create_pixmap = connection.send_request_checked(&x::CreatePixmap {
                depth: screen.root_depth(),
                pid: pixmap,
                drawable: x::Drawable::Window(screen.root()),
                width,
                height,
            });
            connection
                .check_request(create_pixmap)
                .context("could not create the X11 freeze pixmap")?;
            let gc: x::Gcontext = connection.generate_id();
            let create_gc = connection.send_request_checked(&x::CreateGc {
                cid: gc,
                drawable: x::Drawable::Pixmap(pixmap),
                value_list: &[],
            });
            connection
                .check_request(create_gc)
                .context("could not create the X11 freeze graphics context")?;
            let (native, row_bytes) =
                rgba_to_x11(frame.width, frame.height, frame.pixels.as_ref(), format)?;
            let request_bytes = connection.get_maximum_request_length() as usize * 4;
            let rows_per_request = ((request_bytes.saturating_sub(32)) / row_bytes)
                .max(1)
                .min(frame.height as usize);
            let mut image_requests = Vec::new();
            for (index, rows) in native.chunks(row_bytes * rows_per_request).enumerate() {
                let row_count = u16::try_from(rows.len() / row_bytes)
                    .context("freeze image chunk was too tall")?;
                image_requests.push(
                    connection.send_request_checked(&x::PutImage {
                        format: x::ImageFormat::ZPixmap,
                        drawable: x::Drawable::Pixmap(pixmap),
                        gc,
                        width,
                        height: row_count,
                        dst_x: 0,
                        dst_y: i16::try_from(index * rows_per_request)
                            .context("freeze image chunk was outside X11")?,
                        left_pad: 0,
                        depth: screen.root_depth(),
                        data: rows,
                    }),
                );
            }
            for request in image_requests {
                connection
                    .check_request(request)
                    .context("could not upload the X11 freeze image")?;
            }
            connection.send_request(&x::FreeGc { gc });

            let window: x::Window = connection.generate_id();
            let create_window = connection.send_request_checked(&x::CreateWindow {
                depth: screen.root_depth(),
                wid: window,
                parent: screen.root(),
                x,
                y,
                width,
                height,
                border_width: 0,
                class: x::WindowClass::InputOutput,
                visual: screen.root_visual(),
                value_list: &[x::Cw::BackPixmap(pixmap), x::Cw::OverrideRedirect(true)],
            });
            connection
                .check_request(create_window)
                .context("could not create the X11 freeze window")?;
            connection.send_request(&x::ChangeProperty {
                mode: x::PropMode::Replace,
                window,
                property: x::ATOM_WM_NAME,
                r#type: x::ATOM_STRING,
                data: b"Poratake Freeze Screen",
            });
            connection.send_request(&x::MapWindow { window });
            connection.send_request(&x::FreePixmap { pixmap });
            windows.push(window);
        }
        windows
    };
    connection
        .flush()
        .context("could not show X11 freeze windows")?;
    connection
        .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
        .context("could not synchronize X11 freeze windows")?;
    Ok(X11FreezeOverlay {
        connection,
        windows,
    })
}

#[cfg(feature = "wayland")]
fn capture_wayland_frame() -> Result<RgbaImage> {
    let mut capturer = wayland_capturer(1)?;
    let frame = capturer.get_next_frame();
    capturer.stop_capture();
    frame_to_rgba(
        frame.map_err(|error| anyhow!("could not capture a Linux screen frame: {error}"))?,
    )
}

/// Builds a persistent Wayland capturer for the recorder's frame pump. The
/// first frame arrives through the portal session this opens; `get_next_frame`
/// then blocks on the compositor's push, so the pump never polls.
#[cfg(feature = "wayland")]
pub(crate) fn wayland_capturer(fps: u32) -> Result<Capturer> {
    let mut capturer = Capturer::build(Options {
        fps: fps.max(1),
        show_cursor: false,
        show_highlight: false,
        target: None,
        crop_area: None,
        output_type: FrameType::BGRAFrame,
        output_resolution: Resolution::Captured,
        excluded_targets: None,
    })
    .context("could not start Linux screen capture")?;
    capturer.start_capture();
    Ok(capturer)
}

#[cfg(not(feature = "wayland"))]
fn capture_wayland_frame() -> Result<RgbaImage> {
    Err(anyhow!(
        "Wayland capture support was not included in this build"
    ))
}

fn capture_x11_frame(rect: CaptureRect) -> Result<(RgbaImage, DisplayOrigin)> {
    let source = x11_region_source(rect)?;
    let screen = source.screen()?;
    let image = capture_x11_drawable(
        &source.connection,
        screen,
        x::Drawable::Window(screen.root()),
        source.crtc_x,
        source.crtc_y,
        source.crtc_width,
        source.crtc_height,
    )?;
    Ok((image, source.origin))
}

/// The X11 resources needed to grab a display region repeatedly: a live
/// connection, the screen it belongs to, and the output geometry the region
/// sits on. The recorder holds one of these for its frame pump.
pub(crate) struct X11RegionSource {
    pub(crate) connection: xcb::Connection,
    pub(crate) screen_number: usize,
    /// The full output (CRTC) rectangle the capture region sits on.
    pub(crate) crtc_x: i16,
    pub(crate) crtc_y: i16,
    pub(crate) crtc_width: u16,
    pub(crate) crtc_height: u16,
    pub(crate) origin: DisplayOrigin,
}

impl X11RegionSource {
    pub(crate) fn screen(&self) -> Result<&x::Screen> {
        x11_screen(&self.connection, self.screen_number)
    }
}

/// Resolves the X11 output whose display contains `rect`, keeping the
/// connection open so callers can grab the region per frame.
pub(crate) fn x11_region_source(rect: CaptureRect) -> Result<X11RegionSource> {
    let (connection, screen_number) = x11_connection()?;
    let screen = x11_screen(&connection, screen_number)?;
    let resources = connection
        .wait_for_reply(connection.send_request(&GetScreenResources {
            window: screen.root(),
        }))
        .context("could not list X11 displays")?;

    for output in resources.outputs() {
        let info = connection
            .wait_for_reply(connection.send_request(&GetOutputInfo {
                output: *output,
                config_timestamp: resources.config_timestamp(),
            }))
            .context("could not read an X11 display")?;
        if info.connection() != xcb::randr::Connection::Connected {
            continue;
        }

        let crtc = info.crtc();
        if crtc.resource_id() == 0 {
            continue;
        }
        let crtc_info = connection
            .wait_for_reply(connection.send_request(&GetCrtcInfo {
                crtc,
                config_timestamp: resources.config_timestamp(),
            }))
            .context("could not read X11 display geometry")?;
        let origin = DisplayOrigin {
            x: crtc_info.x() as i32,
            y: crtc_info.y() as i32,
        };
        if !contains_region(
            origin,
            (crtc_info.width() as i32, crtc_info.height() as i32),
            rect,
        ) {
            continue;
        }

        return Ok(X11RegionSource {
            connection,
            screen_number,
            crtc_x: crtc_info.x(),
            crtc_y: crtc_info.y(),
            crtc_width: crtc_info.width(),
            crtc_height: crtc_info.height(),
            origin,
        });
    }

    Err(anyhow!("capture area did not fit an X11 display"))
}

pub fn desktop_wallpaper(backend: Backend) -> Result<RgbaImage> {
    if backend != Backend::X11 {
        return Err(anyhow!(
            "desktop wallpaper is only available in X11 sessions"
        ));
    }
    let (connection, screen_number) = x11_connection()?;
    let screen = x11_screen(&connection, screen_number)?;
    for name in ["_XROOTPMAP_ID", "ESETROOT_PMAP_ID"] {
        let Ok(atom) = x11_atom(&connection, name) else {
            continue;
        };
        let Ok(property) = x11_property(&connection, screen.root(), atom) else {
            continue;
        };
        if property.r#type() != x::ATOM_PIXMAP || property.format() != 32 {
            continue;
        }
        let Some(pixmap) = property
            .value::<u32>()
            .first()
            .copied()
            .filter(|id| *id != 0)
        else {
            continue;
        };
        let pixmap = x::Pixmap::new(pixmap);
        let Ok(geometry) = connection.wait_for_reply(connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Pixmap(pixmap),
        })) else {
            continue;
        };
        if geometry.width() == 0 || geometry.height() == 0 {
            continue;
        }
        if let Ok(image) = capture_x11_drawable(
            &connection,
            screen,
            x::Drawable::Pixmap(pixmap),
            0,
            0,
            geometry.width(),
            geometry.height(),
        ) {
            return Ok(image);
        }
    }
    Err(anyhow!("X11 desktop wallpaper pixmap is unavailable"))
}

pub(crate) fn capture_x11_drawable(
    connection: &xcb::Connection,
    screen: &x::Screen,
    drawable: x::Drawable,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Result<RgbaImage> {
    let frame = connection
        .wait_for_reply(connection.send_request(&x::GetImage {
            format: x::ImageFormat::ZPixmap,
            drawable,
            x,
            y,
            width,
            height,
            plane_mask: u32::MAX,
        }))
        .map_err(|error| anyhow!("could not read X11 pixels: {error:?}"))?;
    let visual_id = if frame.visual() == 0 {
        screen.root_visual()
    } else {
        frame.visual()
    };
    x11_to_rgba(
        u32::from(width),
        u32::from(height),
        frame.data(),
        x11_pixel_format(connection, screen, frame.depth(), visual_id)?,
    )
}

fn x11_pixel_format(
    connection: &xcb::Connection,
    screen: &x::Screen,
    depth: u8,
    visual_id: x::Visualid,
) -> Result<X11PixelFormat> {
    let setup = connection.get_setup();
    let pixmap = setup
        .pixmap_formats()
        .iter()
        .find(|format| format.depth() == depth)
        .ok_or_else(|| anyhow!("X11 did not report the captured pixel format"))?;
    let visual = screen
        .allowed_depths()
        .flat_map(|depth| depth.visuals().iter())
        .find(|visual| visual.visual_id() == visual_id)
        .ok_or_else(|| anyhow!("X11 did not report the captured visual"))?;
    Ok(X11PixelFormat {
        bits_per_pixel: pixmap.bits_per_pixel(),
        scanline_pad: pixmap.scanline_pad(),
        lsb_first: setup.image_byte_order() == x::ImageOrder::LsbFirst,
        red_mask: visual.red_mask(),
        green_mask: visual.green_mask(),
        blue_mask: visual.blue_mask(),
    })
}

fn save_png(image: RgbaImage, path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("could not create screenshot directory")?;
    }
    DynamicImage::ImageRgba8(image)
        .save(path)
        .context("could not save Linux screenshot")
}

fn x11_atom(connection: &xcb::Connection, name: &str) -> Result<x::Atom> {
    connection
        .wait_for_reply(connection.send_request(&x::InternAtom {
            only_if_exists: false,
            name: name.as_bytes(),
        }))
        .map(|reply| reply.atom())
        .map_err(|error| anyhow!("could not resolve X11 atom {name}: {error:?}"))
}

fn x11_property(
    connection: &xcb::Connection,
    window: x::Window,
    property: x::Atom,
) -> Result<x::GetPropertyReply> {
    connection
        .wait_for_reply(connection.send_request(&x::GetProperty {
            delete: false,
            window,
            property,
            r#type: x::ATOM_ANY,
            long_offset: 0,
            long_length: 4096,
        }))
        .map_err(|error| anyhow!("could not read X11 window property: {error:?}"))
}

fn x11_text_property(
    connection: &xcb::Connection,
    window: x::Window,
    property: x::Atom,
) -> Result<String> {
    let reply = x11_property(connection, window, property)?;
    Ok(String::from_utf8_lossy(reply.value())
        .trim_matches(char::from(0))
        .trim()
        .to_string())
}

fn x11_window_candidates(
    connection: &xcb::Connection,
    root: x::Window,
    stacking_atom: x::Atom,
) -> Result<Vec<x::Window>> {
    let mut windows = x11_property(connection, root, stacking_atom)?
        .value::<x::Window>()
        .to_vec();
    if windows.is_empty() {
        windows = connection
            .wait_for_reply(connection.send_request(&x::QueryTree { window: root }))
            .context("could not list X11 windows")?
            .children()
            .to_vec();
    }
    windows.reverse();
    Ok(windows)
}

struct WindowAtoms {
    name: x::Atom,
    wm_name: x::Atom,
    class: x::Atom,
    pid: x::Atom,
    window_type: x::Atom,
    desktop_type: x::Atom,
    dock_type: x::Atom,
    stacking: x::Atom,
}

impl WindowAtoms {
    fn new(connection: &xcb::Connection) -> Result<Self> {
        Ok(Self {
            name: x11_atom(connection, "_NET_WM_NAME")?,
            wm_name: x11_atom(connection, "WM_NAME")?,
            class: x11_atom(connection, "WM_CLASS")?,
            pid: x11_atom(connection, "_NET_WM_PID")?,
            window_type: x11_atom(connection, "_NET_WM_WINDOW_TYPE")?,
            desktop_type: x11_atom(connection, "_NET_WM_WINDOW_TYPE_DESKTOP")?,
            dock_type: x11_atom(connection, "_NET_WM_WINDOW_TYPE_DOCK")?,
            stacking: x11_atom(connection, "_NET_CLIENT_LIST_STACKING")?,
        })
    }
}

fn x11_window_info(
    connection: &xcb::Connection,
    root: x::Window,
    window: x::Window,
    atoms: &WindowAtoms,
) -> Result<Option<WindowInfo>> {
    let attributes = connection
        .wait_for_reply(connection.send_request(&x::GetWindowAttributes { window }))
        .context("could not read X11 window attributes")?;
    if attributes.map_state() != x::MapState::Viewable || attributes.override_redirect() {
        return Ok(None);
    }

    let types = x11_property(connection, window, atoms.window_type)?;
    if types
        .value::<x::Atom>()
        .iter()
        .any(|kind| *kind == atoms.desktop_type || *kind == atoms.dock_type)
    {
        return Ok(None);
    }

    let frame_window = x11_frame_window(connection, root, window)?;
    let geometry = connection
        .wait_for_reply(connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Window(frame_window),
        }))
        .context("could not read X11 window geometry")?;
    if geometry.width() < MIN_WINDOW_SIZE || geometry.height() < MIN_WINDOW_SIZE {
        return Ok(None);
    }
    let position = connection
        .wait_for_reply(connection.send_request(&x::TranslateCoordinates {
            src_window: frame_window,
            dst_window: root,
            src_x: 0,
            src_y: 0,
        }))
        .context("could not translate X11 window coordinates")?;

    let owner_pid = x11_property(connection, window, atoms.pid)?
        .value::<u32>()
        .first()
        .copied()
        .unwrap_or_default();
    let owner_name = x11_text_property(connection, window, atoms.class)?
        .split(char::from(0))
        .rfind(|value| !value.is_empty())
        .unwrap_or("Unknown")
        .to_string();
    let mut title = x11_text_property(connection, window, atoms.name)?;
    if title.is_empty() {
        title = x11_text_property(connection, window, atoms.wm_name)?;
    }
    if title.is_empty() {
        title.clone_from(&owner_name);
    }

    Ok(Some(WindowInfo {
        window_id: i64::from(window.resource_id()),
        title,
        owner_name,
        owner_pid: i64::from(owner_pid),
        bounds: WindowBounds {
            x: f64::from(position.dst_x()),
            y: f64::from(position.dst_y()),
            width: f64::from(geometry.width()),
            height: f64::from(geometry.height()),
        },
    }))
}

fn x11_frame_window(
    connection: &xcb::Connection,
    root: x::Window,
    client: x::Window,
) -> Result<x::Window> {
    let mut current = client;
    loop {
        let tree = connection
            .wait_for_reply(connection.send_request(&x::QueryTree { window: current }))
            .context("could not resolve the X11 window frame")?;
        if tree.parent() == root || tree.parent().resource_id() == 0 {
            return Ok(current);
        }
        current = tree.parent();
    }
}

struct RedirectedPixmap<'a> {
    connection: &'a xcb::Connection,
    window: x::Window,
    pixmap: x::Pixmap,
}

impl RedirectedPixmap<'_> {
    fn drawable(&self) -> x::Drawable {
        x::Drawable::Pixmap(self.pixmap)
    }
}

impl Drop for RedirectedPixmap<'_> {
    fn drop(&mut self) {
        self.connection.send_request(&x::FreePixmap {
            pixmap: self.pixmap,
        });
        self.connection.send_request(&composite::UnredirectWindow {
            window: self.window,
            update: composite::Redirect::Automatic,
        });
        let _ = self.connection.flush();
    }
}

fn x11_redirected_pixmap(
    connection: &xcb::Connection,
    window: x::Window,
) -> Result<RedirectedPixmap<'_>> {
    let redirect = connection.send_request_checked(&composite::RedirectWindow {
        window,
        update: composite::Redirect::Automatic,
    });
    connection
        .check_request(redirect)
        .context("could not redirect the X11 window for capture")?;
    let pixmap = connection.generate_id();
    let name = connection.send_request_checked(&composite::NameWindowPixmap { window, pixmap });
    if let Err(error) = connection.check_request(name) {
        connection.send_request(&composite::UnredirectWindow {
            window,
            update: composite::Redirect::Automatic,
        });
        let _ = connection.flush();
        return Err(error).context("could not access the composited X11 window pixels");
    }
    Ok(RedirectedPixmap {
        connection,
        window,
        pixmap,
    })
}

pub fn list_windows(backend: Backend) -> Result<Vec<WindowInfo>> {
    if backend != Backend::X11 {
        return Err(anyhow!(
            "window discovery is only available in X11 sessions"
        ));
    }
    let (connection, screen_number) = x11_connection()?;
    let screen = x11_screen(&connection, screen_number)?;
    let atoms = WindowAtoms::new(&connection)?;
    let candidates = x11_window_candidates(&connection, screen.root(), atoms.stacking)?;
    Ok(candidates
        .into_iter()
        .filter_map(|window| {
            x11_window_info(&connection, screen.root(), window, &atoms)
                .ok()
                .flatten()
        })
        .collect())
}

pub fn capture_window_to_file(backend: Backend, capture: &CaptureWindowRequest) -> Result<()> {
    if backend != Backend::X11 {
        return Err(anyhow!("window capture is only available in X11 sessions"));
    }
    let window_id = u32::try_from(capture.window_id).context("X11 window id was invalid")?;
    let window = x::Window::new(window_id);
    let (connection, screen_number) = x11_connection()?;
    let screen = x11_screen(&connection, screen_number)?;
    let frame_window = x11_frame_window(&connection, screen.root(), window)?;
    let attributes = connection
        .wait_for_reply(connection.send_request(&x::GetWindowAttributes { window }))
        .context("could not read X11 window attributes")?;
    if attributes.map_state() != x::MapState::Viewable {
        return Err(anyhow!("X11 window is not visible"));
    }
    let geometry = connection
        .wait_for_reply(connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Window(frame_window),
        }))
        .context("could not read X11 window geometry")?;
    if geometry.width() == 0 || geometry.height() == 0 {
        return Err(anyhow!("X11 window has no drawable area"));
    }
    let pixmap = x11_redirected_pixmap(&connection, frame_window)?;
    let image = capture_x11_drawable(
        &connection,
        screen,
        pixmap.drawable(),
        0,
        0,
        geometry.width(),
        geometry.height(),
    )?;
    save_png(image, &capture.path)
}

#[derive(Clone, Copy)]
struct X11PixelFormat {
    bits_per_pixel: u8,
    scanline_pad: u8,
    lsb_first: bool,
    red_mask: u32,
    green_mask: u32,
    blue_mask: u32,
}

fn x11_to_rgba(width: u32, height: u32, data: &[u8], format: X11PixelFormat) -> Result<RgbaImage> {
    let bytes_per_pixel = usize::from(format.bits_per_pixel).div_ceil(8);
    if bytes_per_pixel == 0 || bytes_per_pixel > 4 || format.scanline_pad == 0 {
        return Err(anyhow!("X11 returned an unsupported pixel format"));
    }
    let row_bits = width as usize * usize::from(format.bits_per_pixel);
    let pad = usize::from(format.scanline_pad);
    let row_bytes = row_bits.div_ceil(pad) * pad / 8;
    let expected = row_bytes * height as usize;
    if data.len() < expected {
        return Err(anyhow!(
            "X11 frame contained {} bytes, expected at least {expected}",
            data.len()
        ));
    }

    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row in data.chunks_exact(row_bytes).take(height as usize) {
        for pixel in row.chunks_exact(bytes_per_pixel).take(width as usize) {
            let value = if format.lsb_first {
                pixel.iter().enumerate().fold(0u32, |value, (index, byte)| {
                    value | (u32::from(*byte) << (index * 8))
                })
            } else {
                pixel
                    .iter()
                    .fold(0u32, |value, byte| (value << 8) | u32::from(*byte))
            };
            rgba.extend_from_slice(&[
                masked_channel(value, format.red_mask),
                masked_channel(value, format.green_mask),
                masked_channel(value, format.blue_mask),
                255,
            ]);
        }
    }
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| anyhow!("X11 frame was invalid"))
}

fn rgba_to_x11(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: X11PixelFormat,
) -> Result<(Vec<u8>, usize)> {
    let bytes_per_pixel = usize::from(format.bits_per_pixel).div_ceil(8);
    if bytes_per_pixel == 0 || bytes_per_pixel > 4 || format.scanline_pad == 0 {
        return Err(anyhow!("X11 returned an unsupported pixel format"));
    }
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(anyhow!(
            "RGBA frame contained {} bytes, expected {expected}",
            rgba.len()
        ));
    }
    let row_bits = width as usize * usize::from(format.bits_per_pixel);
    let pad = usize::from(format.scanline_pad);
    let row_bytes = row_bits.div_ceil(pad) * pad / 8;
    let mut native = vec![0; row_bytes * height as usize];
    for (row, pixels) in rgba.chunks_exact(width as usize * 4).enumerate() {
        let output = &mut native[row * row_bytes..][..row_bytes];
        for (column, pixel) in pixels.chunks_exact(4).enumerate() {
            let value = channel_to_mask(pixel[0], format.red_mask)
                | channel_to_mask(pixel[1], format.green_mask)
                | channel_to_mask(pixel[2], format.blue_mask);
            let offset = column * bytes_per_pixel;
            if format.lsb_first {
                output[offset..offset + bytes_per_pixel]
                    .copy_from_slice(&value.to_le_bytes()[..bytes_per_pixel]);
            } else {
                output[offset..offset + bytes_per_pixel]
                    .copy_from_slice(&value.to_be_bytes()[4 - bytes_per_pixel..]);
            }
        }
    }
    Ok((native, row_bytes))
}

fn masked_channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    let channel = (value & mask) >> shift;
    ((u64::from(channel) * 255 + u64::from(maximum) / 2) / u64::from(maximum)) as u8
}

fn channel_to_mask(channel: u8, mask: u32) -> u32 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    (((u64::from(channel) * u64::from(maximum) + 127) / 255) as u32) << shift
}

fn contains_region(
    display_origin: DisplayOrigin,
    display_size: (i32, i32),
    region: CaptureRect,
) -> bool {
    region.x >= display_origin.x
        && region.y >= display_origin.y
        && region.x + region.width <= display_origin.x + display_size.0
        && region.y + region.height <= display_origin.y + display_size.1
}

#[cfg(feature = "wayland")]
pub(crate) fn frame_to_rgba(frame: Frame) -> Result<RgbaImage> {
    match frame {
        Frame::RGB(frame) => rgb_to_rgba(frame.width, frame.height, &frame.data),
        Frame::RGBx(frame) => rgbx_to_rgba(frame.width, frame.height, frame.data),
        Frame::XBGR(frame) => xbgr_to_rgba(frame.width, frame.height, frame.data),
        Frame::BGRx(frame) => bgrx_to_rgba(frame.width, frame.height, frame.data),
        Frame::BGR0(frame) => bgrx_to_rgba(frame.width, frame.height, frame.data),
        Frame::BGRA(frame) => bgra_to_rgba(frame.width, frame.height, frame.data),
        Frame::YUVFrame(_) => Err(anyhow!("Linux capture returned an unsupported YUV frame")),
    }
}

#[cfg(feature = "wayland")]
fn dimensions(width: i32, height: i32, channels: usize, length: usize) -> Result<(u32, u32)> {
    let width = u32::try_from(width).context("capture frame width was invalid")?;
    let height = u32::try_from(height).context("capture frame height was invalid")?;
    let expected = width as usize * height as usize * channels;
    if length < expected {
        return Err(anyhow!(
            "capture frame contained {length} bytes, expected at least {expected}"
        ));
    }
    Ok((width, height))
}

#[cfg(feature = "wayland")]
fn rgb_to_rgba(width: i32, height: i32, data: &[u8]) -> Result<RgbaImage> {
    let (width, height) = dimensions(width, height, 3, data.len())?;
    let rgba = data
        .chunks_exact(3)
        .take(width as usize * height as usize)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
        .collect();
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| anyhow!("capture frame was invalid"))
}

#[cfg(feature = "wayland")]
fn rgbx_to_rgba(width: i32, height: i32, mut data: Vec<u8>) -> Result<RgbaImage> {
    let (width, height) = dimensions(width, height, 4, data.len())?;
    data.truncate(width as usize * height as usize * 4);
    for pixel in data.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    RgbaImage::from_raw(width, height, data).ok_or_else(|| anyhow!("capture frame was invalid"))
}

#[cfg(feature = "wayland")]
fn xbgr_to_rgba(width: i32, height: i32, mut data: Vec<u8>) -> Result<RgbaImage> {
    let (width, height) = dimensions(width, height, 4, data.len())?;
    data.truncate(width as usize * height as usize * 4);
    for pixel in data.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[pixel[3], pixel[2], pixel[1], 255]);
    }
    RgbaImage::from_raw(width, height, data).ok_or_else(|| anyhow!("capture frame was invalid"))
}

#[cfg(feature = "wayland")]
fn bgrx_to_rgba(width: i32, height: i32, mut data: Vec<u8>) -> Result<RgbaImage> {
    let (width, height) = dimensions(width, height, 4, data.len())?;
    data.truncate(width as usize * height as usize * 4);
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    RgbaImage::from_raw(width, height, data).ok_or_else(|| anyhow!("capture frame was invalid"))
}

#[cfg(feature = "wayland")]
fn bgra_to_rgba(width: i32, height: i32, mut data: Vec<u8>) -> Result<RgbaImage> {
    let (width, height) = dimensions(width, height, 4, data.len())?;
    data.truncate(width as usize * height as usize * 4);
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    RgbaImage::from_raw(width, height, data).ok_or_else(|| anyhow!("capture frame was invalid"))
}

fn crop(image: &RgbaImage, rect: CaptureRect) -> Result<RgbaImage> {
    let x = u32::try_from(rect.x).context("capture area starts outside the selected display")?;
    let y = u32::try_from(rect.y).context("capture area starts outside the selected display")?;
    let width = u32::try_from(rect.width).context("capture area width was invalid")?;
    let height = u32::try_from(rect.height).context("capture area height was invalid")?;
    if x + width > image.width() || y + height > image.height() {
        return Err(anyhow!(
            "capture area does not fit the portal source; select the same display in the system prompt"
        ));
    }
    Ok(image::imageops::crop_imm(image, x, y, width, height).to_image())
}

fn crop_on_display(
    image: &RgbaImage,
    rect: CaptureRect,
    display_origin: DisplayOrigin,
) -> Result<RgbaImage> {
    crop(
        image,
        CaptureRect {
            x: rect.x - display_origin.x,
            y: rect.y - display_origin.y,
            ..rect
        },
    )
}

fn crop_frozen_frame(frame: &FrozenFrame, rect: CaptureRect) -> Option<RgbaImage> {
    let x = usize::try_from(rect.x - frame.origin.x).ok()?;
    let y = usize::try_from(rect.y - frame.origin.y).ok()?;
    let width = usize::try_from(rect.width).ok()?;
    let height = usize::try_from(rect.height).ok()?;
    let stride = frame.width as usize * 4;
    let mut pixels = Vec::with_capacity(width * height * 4);
    for row in y..y + height {
        let start = row * stride + x * 4;
        pixels.extend_from_slice(&frame.pixels.as_ref()[start..start + width * 4]);
    }
    RgbaImage::from_raw(width as u32, height as u32, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> CaptureRect {
        CaptureRect {
            x,
            y,
            width,
            height,
        }
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn bgrx_frames_convert_to_opaque_rgba() {
        let image = bgrx_to_rgba(1, 1, vec![3, 2, 1, 0]).expect("convert frame");
        assert_eq!(image.into_raw(), vec![1, 2, 3, 255]);
    }

    #[test]
    fn crops_the_requested_region() {
        let image = RgbaImage::from_raw(
            2,
            2,
            vec![1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255],
        )
        .expect("image");
        let cropped = crop(&image, rect(1, 0, 1, 2)).expect("crop");
        assert_eq!(cropped.into_raw(), vec![2, 0, 0, 255, 4, 0, 0, 255]);
    }

    #[test]
    fn rejects_a_region_outside_the_captured_source() {
        let image = RgbaImage::new(10, 10);
        assert!(crop(&image, rect(8, 8, 3, 3)).is_err());
    }

    #[test]
    fn normalizes_a_secondary_display_region_to_the_portal_source() {
        let image = RgbaImage::from_raw(2, 1, vec![1, 0, 0, 255, 2, 0, 0, 255]).expect("image");
        let cropped = crop_on_display(&image, rect(1921, 0, 1, 1), DisplayOrigin { x: 1920, y: 0 })
            .expect("crop");
        assert_eq!(cropped.into_raw(), vec![2, 0, 0, 255]);
    }

    #[test]
    fn frozen_frames_crop_in_virtual_desktop_coordinates() {
        let frozen = FrozenFrames::default();
        frozen.replace(vec![FrozenFrame {
            width: 2,
            height: 1,
            pixels: SharedPixels(Arc::new(vec![1, 0, 0, 255, 2, 0, 0, 255])),
            origin: DisplayOrigin { x: -2, y: 5 },
        }]);

        let cropped = frozen.crop(rect(-1, 5, 1, 1)).expect("cached crop");
        assert_eq!(cropped.into_raw(), vec![2, 0, 0, 255]);
    }

    #[test]
    fn matches_a_region_to_its_x11_display() {
        assert!(contains_region(
            DisplayOrigin { x: 1920, y: 0 },
            (2560, 1440),
            rect(2000, 100, 800, 600),
        ));
        assert!(!contains_region(
            DisplayOrigin { x: 1920, y: 0 },
            (2560, 1440),
            rect(1800, 100, 800, 600),
        ));
    }

    #[test]
    fn decodes_little_endian_rgb565() {
        let image = x11_to_rgba(
            1,
            1,
            &[0x00, 0xf8],
            X11PixelFormat {
                bits_per_pixel: 16,
                scanline_pad: 16,
                lsb_first: true,
                red_mask: 0xf800,
                green_mask: 0x07e0,
                blue_mask: 0x001f,
            },
        )
        .expect("decode RGB565");

        assert_eq!(image.into_raw(), vec![255, 0, 0, 255]);
    }

    #[test]
    fn x11_round_trip_preserves_checker_pixels_and_padding() {
        let format = X11PixelFormat {
            bits_per_pixel: 32,
            scanline_pad: 32,
            lsb_first: true,
            red_mask: 0x00ff_0000,
            green_mask: 0x0000_ff00,
            blue_mask: 0x0000_00ff,
        };
        let rgba = vec![255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        let (native, _) = rgba_to_x11(3, 1, &rgba, format).expect("encode X11");
        let decoded = x11_to_rgba(3, 1, &native, format).expect("decode X11");

        assert_eq!(decoded.into_raw(), rgba);
    }

    #[test]
    fn decodes_ten_bit_x11_channels() {
        let value = 0x3ff0_0000u32;
        let image = x11_to_rgba(
            1,
            1,
            &value.to_le_bytes(),
            X11PixelFormat {
                bits_per_pixel: 32,
                scanline_pad: 32,
                lsb_first: true,
                red_mask: 0x3ff0_0000,
                green_mask: 0x000f_fc00,
                blue_mask: 0x0000_03ff,
            },
        )
        .expect("decode 30-bit color");

        assert_eq!(image.into_raw(), vec![255, 0, 0, 255]);
    }
}
