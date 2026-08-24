//! Area selection overlay — a fullscreen transparent window per the ported
//! `area-overlay` flow: dimmed screen, drag to draw the selection, Esc to
//! cancel, release to confirm and capture.

use gpui::{
    actions, div, prelude::*, px, AnyWindowHandle, App, Bounds, Context, DisplayId, Global,
    KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render,
    Styled, Window, WindowBackgroundAppearance, WindowKind, WindowOptions,
};

use crate::capture::selection;
use crate::capture::CaptureService;
use crate::theme::color::Srgba;

actions!(overlay, [Cancel]);

pub fn init_bindings(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some("AreaOverlay"))]);
}

/// Which pointer gesture is in flight, mirroring the `Interaction` union in
/// `use-area-selection.ts`.
#[derive(Clone, Copy, Debug)]
enum Interaction {
    Creating { start: selection::Point },
    Moving { offset: selection::Point },
    Resizing { handle: selection::Handle },
}

/// A confirmed selection in global *physical* pixels.
#[derive(Clone, Copy, Debug)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Default)]
struct OverlaySession {
    handles: Vec<AnyWindowHandle>,
}

impl Global for OverlaySession {}

fn session(cx: &mut App) -> &mut OverlaySession {
    cx.default_global::<OverlaySession>()
}

pub fn close_all(cx: &mut App) {
    let handles = std::mem::take(&mut session(cx).handles);
    if !handles.is_empty() {
        // The daemon holds the frozen displays until it is told otherwise, so
        // the release is tied to the overlay going away rather than to the
        // capture succeeding.
        release_frozen_screen(cx);
    }
    App::defer(cx, move |cx| {
        for handle in handles {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    });
}

/// Drops the daemon's frozen snapshot. Cheap and idempotent when the screen was
/// never frozen, which is why the callers do not track whether it was.
fn release_frozen_screen(cx: &mut App) {
    let service = crate::state::state(cx);
    cx.background_executor()
        .spawn(async move { service.release_screen() })
        .detach();
}

fn dismiss(window: &mut Window, cx: &mut App) {
    close_all(cx);
    window.remove_window();
}

pub fn app_scale_factor(cx: &mut App) -> f32 {
    cx.windows()
        .into_iter()
        .find_map(|handle| handle.update(cx, |_, window, _| window.scale_factor()).ok())
        .unwrap_or(1.0)
}

pub fn physical_rect(bounds: Bounds<Pixels>, scale: f32) -> ScreenRect {
    let scale = scale.max(0.01);
    ScreenRect {
        x: (f32::from(bounds.origin.x) * scale).round() as i32,
        y: (f32::from(bounds.origin.y) * scale).round() as i32,
        width: (f32::from(bounds.size.width) * scale).round() as i32,
        height: (f32::from(bounds.size.height) * scale).round() as i32,
    }
}

fn overlay_options(
    display_bounds: Bounds<Pixels>,
    display_id: DisplayId,
    focus: bool,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(display_bounds)),
        titlebar: None,
        focus,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display_id),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}

fn track_overlay(handle: AnyWindowHandle, cx: &mut App) {
    session(cx).handles.push(handle);
}

fn begin_recording(
    cx: &mut App,
    target: crate::video::recorder::RecordingTarget,
    rect: ScreenRect,
    window_id: Option<i64>,
    target_name: Option<String>,
) {
    crate::windows::recording_control::RecordingControl::open(
        cx,
        target,
        rect,
        window_id,
        target_name,
    );
}

pub struct AreaOverlay {
    pub focus_handle: gpui::FocusHandle,
    display_bounds: Bounds<Pixels>,
    scale: f32,
    intent: crate::capture::intent::CaptureIntent,
    /// Non-empty in window-pick mode: hovering highlights a window and a
    /// click captures it, mirroring the Electron overlay's pick targets.
    windows: Vec<crate::capture::windows_list::WindowListItem>,
    hovered_window: Option<usize>,
    /// Last pointer position in client coordinates, for `CrosshairGuides`.
    pointer: Option<Point<Pixels>>,
    /// The live selection, which stays editable after it is drawn — the
    /// renderer keeps a box the user can move, resize and re-capture.
    rect: Option<selection::Rect>,
    interaction: Option<Interaction>,
    cursor: gpui::CursorStyle,
    /// `autoConfirm` in `area-overlay/session.ts`: the plain capture flows
    /// close on release, the all-in-one flow keeps the overlay up so the box
    /// can be adjusted.
    auto_confirm: bool,
    /// Set in all-in-one mode: the toolbar's current mode and target.
    all_in_one: Option<crate::capture::all_in_one::Choices>,
    menu: crate::ui::menu::MenuHandle,
    service: CaptureService,
}

impl AreaOverlay {
    pub fn with_focus(
        display_bounds: Bounds<Pixels>,
        scale: f32,
        service: CaptureService,
        intent: crate::capture::intent::CaptureIntent,
        focus_handle: gpui::FocusHandle,
    ) -> Self {
        Self {
            focus_handle,
            display_bounds,
            scale,
            pointer: None,
            rect: None,
            interaction: None,
            cursor: gpui::CursorStyle::Crosshair,
            auto_confirm: true,
            intent,
            windows: Vec::new(),
            hovered_window: None,
            all_in_one: None,
            menu: crate::ui::menu::MenuHandle::new(),
            service,
        }
    }

    pub fn with_windows(
        mut self,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
    ) -> Self {
        self.windows = windows;
        self
    }

    pub fn with_all_in_one(mut self, choices: crate::capture::all_in_one::Choices) -> Self {
        self.all_in_one = Some(choices);
        // `startAreaSelection` passes `autoConfirm: false` for this flow, so a
        // capture leaves the overlay up and the box stays adjustable.
        self.auto_confirm = false;
        self
    }

    fn is_picking_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    /// The hovered window's frame, converted from physical screen pixels into
    /// this overlay's logical client coordinates.
    fn hovered_frame(&self) -> Option<(Point<Pixels>, gpui::Size<Pixels>)> {
        let window = self.windows.get(self.hovered_window?)?;
        let scale = self.scale.max(0.01);
        let left = px(window.bounds.x as f32 / scale) - self.display_bounds.left();
        let top = px(window.bounds.y as f32 / scale) - self.display_bounds.top();
        Some((
            point(left, top),
            size(
                px(window.bounds.width as f32 / scale),
                px(window.bounds.height as f32 / scale),
            ),
        ))
    }

    fn confirm_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.hovered_window else {
            return;
        };
        let Some(picked) = self.windows.get(index).cloned() else {
            return;
        };
        if self.intent == crate::capture::intent::CaptureIntent::Recording {
            let rect = ScreenRect {
                x: picked.bounds.x as i32,
                y: picked.bounds.y as i32,
                width: picked.bounds.width as i32,
                height: picked.bounds.height as i32,
            };
            let label = picked.label();
            let window_id = picked.window_id;
            begin_recording(
                cx,
                crate::video::recorder::RecordingTarget::Window,
                rect,
                Some(window_id),
                Some(label),
            );
            dismiss(window, cx);
            return;
        }
        let coordinator = crate::state::coordinator(cx);
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_window(picked.window_id, cx);
        });
        dismiss(window, cx);
    }

    /// Opens the overlay covering one display.
    pub fn open(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        focus: bool,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        let bounds_for_entity = display_bounds;
        let opened = cx.open_window(
            overlay_options(display_bounds, display_id, focus),
            |window, cx| {
                let scale = window.scale_factor();
                let focus = cx.focus_handle();
                let overlay =
                    AreaOverlay::with_focus(bounds_for_entity, scale, service, intent, focus);
                window.focus(&overlay.focus_handle);
                cx.new(|_| overlay)
            },
        );
        match opened {
            Ok(handle) => {
                track_overlay(handle.into(), cx);
                Some(handle)
            }
            Err(error) => {
                eprintln!("[overlay] failed to open: {error}");
                None
            }
        }
    }

    /// All-in-one variant of `open`: the overlay carries the toolbar and
    /// routes the confirmed selection through the picked mode.
    pub fn open_all_in_one(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        choices: crate::capture::all_in_one::Choices,
        focus: bool,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        let opened = cx.open_window(
            overlay_options(display_bounds, display_id, focus),
            |window, cx| {
                let scale = window.scale_factor();
                let focus = cx.focus_handle();
                let overlay = AreaOverlay::with_focus(
                    display_bounds,
                    scale,
                    service,
                    crate::capture::intent::CaptureIntent::Screenshot,
                    focus,
                )
                .with_all_in_one(choices);
                window.focus(&overlay.focus_handle);
                cx.new(|_| overlay)
            },
        );
        match opened {
            Ok(handle) => {
                track_overlay(handle.into(), cx);
                Some(handle)
            }
            Err(error) => {
                eprintln!("[overlay] failed to open all-in-one: {error}");
                None
            }
        }
    }

    pub fn set_all_in_one_mode(
        &mut self,
        mode: crate::capture::all_in_one::Mode,
        cx: &mut Context<Self>,
    ) {
        use crate::capture::all_in_one::Mode as AioMode;
        use crate::capture::intent::CaptureIntent;

        let Some(choices) = &mut self.all_in_one else {
            return;
        };
        choices.mode = mode;
        let choices = *choices;
        self.intent = match mode {
            AioMode::Ocr => CaptureIntent::Ocr,
            AioMode::Record => CaptureIntent::Recording,
            AioMode::Screenshot => CaptureIntent::Screenshot,
        };
        crate::capture::all_in_one::remember(&self.service.config, choices);
        self.apply_all_in_one_target(cx);
        cx.notify();
    }

    pub fn set_all_in_one_target(
        &mut self,
        target: crate::capture::all_in_one::Target,
        cx: &mut Context<Self>,
    ) {
        let Some(choices) = &mut self.all_in_one else {
            return;
        };
        choices.target = target;
        let choices = *choices;
        crate::capture::all_in_one::remember(&self.service.config, choices);
        self.apply_all_in_one_target(cx);
        cx.notify();
    }

    /// A window target turns the overlay into the window picker; area and
    /// screen keep the drag selection.
    fn apply_all_in_one_target(&mut self, cx: &mut Context<Self>) {
        use crate::capture::all_in_one::Target;

        let Some(choices) = self.all_in_one else {
            return;
        };
        self.rect = None;
        self.interaction = None;
        self.cursor = gpui::CursorStyle::Crosshair;
        self.hovered_window = None;
        if choices.target == Target::Window {
            self.windows.clear();
            let daemon = self.service.daemon.clone();
            cx.spawn(async move |entity, cx| {
                let windows = cx
                    .background_executor()
                    .spawn(async move { crate::capture::windows_list::list(&daemon) })
                    .await;
                let _ = entity.update(cx, |this, cx| {
                    this.windows = windows;
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.windows = Vec::new();
        }
        cx.notify();
    }

    /// Copies the colour under the cursor, matching the overlay's eyedropper.
    pub fn pick_color_under_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self.pointer else {
            crate::windows::toast::Toast::show(
                cx,
                "Move the pointer first",
                "Hover the pixel you want before picking a colour",
            );
            return;
        };
        let scale = self.scale.max(0.01);
        let x = (f32::from(self.display_bounds.left() + position.x) * scale).round() as i32;
        let y = (f32::from(self.display_bounds.top() + position.y) * scale).round() as i32;

        let service = self.service.clone();
        cx.spawn(async move |_entity, cx| {
            let sampled = cx
                .background_executor()
                .spawn(async move {
                    let path = std::env::temp_dir().join(format!("poratake-pick-{x}-{y}.png"));
                    let sampled = service
                        .capture_area_to_file(x, y, 1, 1, &path)
                        .ok()
                        .and_then(|_| image::open(&path).ok())
                        .map(|image| image.to_rgba8());
                    let _ = std::fs::remove_file(&path);
                    sampled
                })
                .await;
            let _ = cx.update(|cx| {
                let Some(sampled) = sampled else {
                    crate::windows::toast::Toast::show(
                        cx,
                        "Pick failed",
                        "Could not read that pixel",
                    );
                    return;
                };
                let pixel = sampled.get_pixel(0, 0).0;
                let hex = format!("#{:02x}{:02x}{:02x}", pixel[0], pixel[1], pixel[2]);
                let _ = arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(hex.clone()));
                crate::windows::toast::Toast::show(cx, "Colour copied", hex);
            });
        })
        .detach();
    }

    /// Window-pick variant of `open`.
    pub fn open_with_windows(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
        focus: bool,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        let opened = cx.open_window(
            overlay_options(display_bounds, display_id, focus),
            |window, cx| {
                let scale = window.scale_factor();
                let focus = cx.focus_handle();
                let overlay =
                    AreaOverlay::with_focus(display_bounds, scale, service, intent, focus)
                        .with_windows(windows);
                window.focus(&overlay.focus_handle);
                cx.new(|_| overlay)
            },
        );
        match opened {
            Ok(handle) => {
                track_overlay(handle.into(), cx);
                Some(handle)
            }
            Err(error) => {
                eprintln!("[overlay] failed to open window picker: {error}");
                None
            }
        }
    }

    fn selection(&self) -> Option<(Point<Pixels>, gpui::Size<Pixels>)> {
        let rect = self.rect?;
        Some((
            point(px(rect.x), px(rect.y)),
            size(px(rect.width), px(rect.height)),
        ))
    }

    /// The display in the units `selection` works in.
    fn viewport(&self) -> selection::Size {
        selection::Size {
            width: f32::from(self.display_bounds.size.width),
            height: f32::from(self.display_bounds.size.height),
        }
    }

    fn to_selection_point(position: Point<Pixels>) -> selection::Point {
        selection::Point {
            x: f32::from(position.x),
            y: f32::from(position.y),
        }
    }

    /// `dragTo` in `use-area-selection.ts`.
    fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(interaction) = self.interaction else {
            return;
        };
        let bounds = self.viewport();
        let point = selection::clamp_point(Self::to_selection_point(position), bounds);
        // The renderer constrains the box when a caller sets an aspect-ratio
        // preset over IPC (`setAreaSelectorAspectRatio`). No GPUI flow supplies
        // one yet, so the geometry runs unconstrained; `selection::resize_rect`
        // takes the ratio the day one does.
        let ratio: Option<f32> = None;

        let next = match interaction {
            Interaction::Creating { start } => {
                let created = selection::normalize_rect(start, point);
                match ratio {
                    Some(ratio) => selection::fit_rect(
                        selection::adjust_rect_to_ratio(created, ratio, None),
                        bounds,
                    ),
                    None => created,
                }
            }
            Interaction::Moving { offset } => {
                let Some(current) = self.rect else { return };
                selection::move_rect(current, point, offset, bounds)
            }
            Interaction::Resizing { handle } => {
                let Some(current) = self.rect else { return };
                selection::fit_rect(
                    selection::resize_rect(current, point, handle, ratio),
                    bounds,
                )
            }
        };

        self.rect = Some(next);
        self.cursor = selection::cursor_for(self.rect, point);
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((origin, size)) = self.selection() else {
            return;
        };
        if size.width < px(4.0) || size.height < px(4.0) {
            return;
        }

        // Logical client coords → physical screen pixels.
        let x = (f32::from(self.display_bounds.left() + origin.x) * self.scale).round() as i32;
        let y = (f32::from(self.display_bounds.top() + origin.y) * self.scale).round() as i32;
        let width = (f32::from(size.width) * self.scale).round() as i32;
        let height = (f32::from(size.height) * self.scale).round() as i32;

        let intent = self.intent;
        if intent == crate::capture::intent::CaptureIntent::Recording {
            begin_recording(
                cx,
                crate::video::recorder::RecordingTarget::Area,
                ScreenRect {
                    x,
                    y,
                    width,
                    height,
                },
                None,
                None,
            );
            dismiss(window, cx);
            return;
        }

        let coordinator = crate::state::coordinator(cx);
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_area_for(
                ScreenRect {
                    x,
                    y,
                    width,
                    height,
                },
                intent,
                cx,
            );
        });
        if self.auto_confirm {
            dismiss(window, cx);
        } else {
            // `handleSelected` in the all-in-one flow captures and leaves the
            // overlay up, so the box can be nudged and captured again.
            cx.notify();
        }
    }
}

/// The hint the Electron overlay shows while nothing is selected yet.
fn prompt(text: &'static str, toolbar: bool) -> gpui::AnyElement {
    use crate::ui::chrome;
    let top = chrome::overlay_prompt_top(toolbar);
    div()
        .absolute()
        .top(px(top))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .rounded_full()
                .bg(Srgba::parse("#000000").to_hsla().opacity(0.7))
                .px(px(chrome::OVERLAY_PROMPT_PX))
                .py(px(chrome::OVERLAY_PROMPT_PY))
                .text_size(px(chrome::OVERLAY_PROMPT_SIZE))
                .text_color(crate::ui::colors::white(1.0))
                .shadow_lg()
                .child(text),
        )
        .into_any_element()
}

fn point(x: Pixels, y: Pixels) -> Point<Pixels> {
    gpui::point(x, y)
}

/// `CrosshairGuides`: two 1px `bg-primary/70` rules through the pointer, shown
/// while nothing is selected and no drag is in progress.
fn crosshair_guides(pointer: Point<Pixels>, accent: gpui::Hsla) -> gpui::AnyElement {
    let rule = accent.opacity(0.7);
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(pointer.x)
                .w(px(1.0))
                .bg(rule),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(pointer.y)
                .h(px(1.0))
                .bg(rule),
        )
        .into_any_element()
}

/// `SelectionScrim` with a rect: four dim bars around the hole, leaving the
/// selected (or hovered) region undimmed.
fn scrim_around(
    origin: Point<Pixels>,
    hole: gpui::Size<Pixels>,
    viewport: gpui::Size<Pixels>,
    dim: gpui::Hsla,
) -> Vec<gpui::AnyElement> {
    let right = origin.x + hole.width;
    let bottom = origin.y + hole.height;
    vec![
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(origin.y.max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(bottom)
            .h((viewport.height - bottom).max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .top(origin.y)
            .w(origin.x.max(px(0.0)))
            .h(hole.height)
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left(right)
            .top(origin.y)
            .w((viewport.width - right).max(px(0.0)))
            .h(hole.height)
            .bg(dim)
            .into_any_element(),
    ]
}

fn size(width: Pixels, height: Pixels) -> gpui::Size<Pixels> {
    gpui::size(width, height)
}

impl Render for AreaOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::vars::active_theme(cx);
        // The overlay's frame, handles and crosshairs are `border-primary` /
        // `bg-primary` / `bg-primary/70`, and `--primary` is the operating
        // system's accent rather than the theme's -- see `theme::vars`.
        let accent = theme.primary;

        let selection = self.selection();
        let dim = Srgba::parse("#000000")
            .to_hsla()
            .opacity(crate::ui::chrome::OVERLAY_DIM);

        let mut root = div()
            .id("area-overlay")
            .size_full()
            .key_context("AreaOverlay")
            .track_focus(&self.focus_handle)
            .cursor(self.cursor)
            .on_action(cx.listener(|_this, _: &Cancel, window, cx| {
                dismiss(window, cx);
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up));

        // Dim everything; punch a "hole" by drawing four bars around it.
        if let Some(choices) = self.all_in_one {
            root = root.child(crate::capture::all_in_one_toolbar::render(
                choices, &self.menu, &theme, cx,
            ));
        }

        if self.is_picking_windows() {
            // `SelectionScrim` takes the hovered target as its rect while
            // picking, so the candidate window shows through undimmed. The DOM
            // overlay draws neither a frame nor a name label in this mode.
            let viewport = window.viewport_size();
            let mut picking = root.cursor_pointer();
            picking = match self.hovered_frame() {
                Some((origin, frame)) => {
                    picking.children(scrim_around(origin, frame, viewport, dim))
                }
                None => picking.child(div().absolute().inset_0().bg(dim)),
            };
            // While picking a window the reference shows the pick-targets
            // prompt, not the drag one.
            return picking.child(prompt(
                crate::capture::intent::WINDOW_PICK_PROMPT,
                self.all_in_one.is_some(),
            ));
        }

        let element = match selection {
            None => root
                .child(div().absolute().inset_0().bg(dim))
                .children(
                    self.pointer
                        .map(|pointer| crosshair_guides(pointer, accent)),
                )
                .child(prompt(self.intent.prompt(), self.all_in_one.is_some())),
            Some((origin, bounds_size)) => {
                let left_w = origin.x;
                let right_x = origin.x + bounds_size.width;
                let right_w = px(f32::from(window.viewport_size().width) - f32::from(right_x));
                let bottom_y = origin.y + bounds_size.height;
                let bottom_h = px(f32::from(window.viewport_size().height) - f32::from(bottom_y));

                root.child(div().absolute().left_0().top_0().w(left_w).h_full().bg(dim))
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(right_w.max(px(0.0)))
                            .h_full()
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(origin.x)
                            .top_0()
                            .w(bounds_size.width)
                            .h(origin.y)
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(origin.x)
                            .top(bottom_y)
                            .w(bounds_size.width)
                            .h(bottom_h.max(px(0.0)))
                            .bg(dim),
                    )
                    .child({
                        let mut frame = div()
                            .absolute()
                            .left(origin.x)
                            .top(origin.y)
                            .w(bounds_size.width)
                            .h(bounds_size.height)
                            .border_1()
                            .border_color(accent)
                            // `shadow-[0_0_0_1px_rgba(0,0,0,0.35)]` keeps the
                            // accent frame legible over light content.
                            .shadow(vec![gpui::BoxShadow {
                                color: crate::ui::colors::black(0.35),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(1.0),
                            }]);
                        for (handle_x, handle_y, handle_w, handle_h) in
                            crate::ui::chrome::overlay_handle_rects(
                                f32::from(bounds_size.width),
                                f32::from(bounds_size.height),
                            )
                        {
                            frame = frame.child(
                                div()
                                    .absolute()
                                    .left(px(handle_x))
                                    .top(px(handle_y))
                                    .w(px(handle_w))
                                    .h(px(handle_h))
                                    .bg(accent)
                                    // `ring-1 ring-black/20` on every bar.
                                    .shadow(vec![gpui::BoxShadow {
                                        color: crate::ui::colors::black(0.2),
                                        offset: point(px(0.0), px(0.0)),
                                        blur_radius: px(0.0),
                                        spread_radius: px(1.0),
                                    }]),
                            );
                        }
                        frame
                    })
                    .child({
                        let viewport_h = f32::from(window.viewport_size().height);
                        let top = f32::from(origin.y);
                        let height = f32::from(bounds_size.height);
                        let label_y = crate::ui::chrome::overlay_label_top(top, height, viewport_h);
                        div()
                            .absolute()
                            .left(origin.x)
                            .w(bounds_size.width)
                            .top(px(label_y))
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .rounded(px(crate::ui::chrome::OVERLAY_LABEL_RADIUS))
                                    .bg(crate::ui::colors::black(0.75))
                                    .px(px(crate::ui::chrome::OVERLAY_LABEL_PX))
                                    .py(px(crate::ui::chrome::OVERLAY_LABEL_PY))
                                    .text_size(px(crate::ui::chrome::OVERLAY_LABEL_SIZE))
                                    .text_color(crate::ui::colors::white(1.0))
                                    .font_family(crate::ui::colors::MONO_FONT)
                                    .child(format!(
                                        "{} × {}",
                                        (f32::from(bounds_size.width * self.scale)).round() as i32,
                                        (f32::from(bounds_size.height * self.scale)).round() as i32
                                    )),
                            )
                    })
            }
        };

        element
    }
}

impl AreaOverlay {
    /// `startDrag`: a press on a handle resizes, inside the box moves, and
    /// anywhere else starts a new box.
    fn on_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_picking_windows() {
            return;
        }
        if self
            .all_in_one
            .is_some_and(|choices| choices.target == crate::capture::all_in_one::Target::Screen)
        {
            self.confirm_screen(window, cx);
            return;
        }

        let bounds = self.viewport();
        let point = selection::clamp_point(Self::to_selection_point(event.position), bounds);

        match selection::gesture_at(self.rect, point) {
            selection::Gesture::Resize { handle } => {
                self.interaction = Some(Interaction::Resizing { handle });
                self.cursor = handle.cursor();
            }
            selection::Gesture::Move { offset } => {
                self.interaction = Some(Interaction::Moving { offset });
                self.cursor = gpui::CursorStyle::ClosedHand;
            }
            selection::Gesture::Create => {
                self.interaction = Some(Interaction::Creating { start: point });
                self.rect = None;
                self.cursor = gpui::CursorStyle::Crosshair;
            }
        }
        cx.notify();
    }

    fn confirm_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scale = self.scale;
        let rect = ScreenRect {
            x: (f32::from(self.display_bounds.left()) * scale).round() as i32,
            y: (f32::from(self.display_bounds.top()) * scale).round() as i32,
            width: (f32::from(self.display_bounds.size.width) * scale).round() as i32,
            height: (f32::from(self.display_bounds.size.height) * scale).round() as i32,
        };
        let intent = self.intent;
        if intent == crate::capture::intent::CaptureIntent::Recording {
            begin_recording(
                cx,
                crate::video::recorder::RecordingTarget::Screen,
                rect,
                None,
                None,
            );
            dismiss(window, cx);
            return;
        }
        let coordinator = crate::state::coordinator(cx);
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_area_for(rect, intent, cx);
        });
        dismiss(window, cx);
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.pointer != Some(event.position) {
            self.pointer = Some(event.position);
            if !self.is_picking_windows() && self.selection().is_none() {
                cx.notify();
            }
        }
        if self.is_picking_windows() {
            let scale = self.scale.max(0.01);
            let x = f32::from(self.display_bounds.left() + event.position.x) * scale;
            let y = f32::from(self.display_bounds.top() + event.position.y) * scale;
            let hovered = crate::capture::windows_list::hit_test(&self.windows, x as f64, y as f64)
                .and_then(|picked| {
                    self.windows
                        .iter()
                        .position(|window| window.window_id == picked.window_id)
                });
            if hovered != self.hovered_window {
                self.hovered_window = hovered;
                cx.notify();
            }
            return;
        }
        if self.interaction.is_some() {
            self.drag_to(event.position, cx);
            return;
        }

        // With no gesture in flight the cursor still tracks the box, so the
        // move and resize affordances are discoverable.
        let bounds = self.viewport();
        let point = selection::clamp_point(Self::to_selection_point(event.position), bounds);
        let cursor = selection::cursor_for(self.rect, point);
        if cursor != self.cursor {
            self.cursor = cursor;
            cx.notify();
        }
    }

    /// `endDrag`: a freshly drawn box is committed (or discarded when it is too
    /// small); an adjusted one stays put.
    fn on_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_picking_windows() {
            self.confirm_window(window, cx);
            return;
        }
        let Some(interaction) = self.interaction.take() else {
            return;
        };
        self.drag_to(event.position, cx);

        let bounds = self.viewport();
        let point = selection::clamp_point(Self::to_selection_point(event.position), bounds);
        self.cursor = selection::cursor_for(self.rect, point);

        if !matches!(interaction, Interaction::Creating { .. }) {
            cx.notify();
            return;
        }

        match self.rect {
            Some(rect) if selection::is_usable_selection(rect) => self.confirm(window, cx),
            _ => {
                self.rect = None;
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{px, size};

    use crate::ui::chrome;

    #[test]
    fn overlay_dim_label_and_handles_match_electron() {
        assert_eq!(chrome::OVERLAY_DIM, 0.5);
        assert_eq!(chrome::OVERLAY_FRAME_BORDER, 1.0);
        assert!(chrome::overlay_label_below(20.0, 80.0, 200.0));
        assert!(!chrome::overlay_label_below(150.0, 40.0, 200.0));
        assert_eq!(chrome::overlay_label_top(20.0, 80.0, 200.0), 108.0);
        assert_eq!(chrome::overlay_label_top(150.0, 40.0, 200.0), 154.0);
        let handles = chrome::overlay_handle_rects(100.0, 80.0);
        assert_eq!(handles.len(), 12);
        assert_eq!(handles[0], (0.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[1], (0.0, 0.0, 4.0, 20.0));
        assert_eq!(handles[2], (80.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[8], (40.0, 0.0, 20.0, 4.0));
    }

    #[test]
    fn physical_rect_multiplies_logical_bounds_by_scale() {
        let bounds = gpui::Bounds {
            origin: gpui::point(px(100.0), px(50.0)),
            size: size(px(800.0), px(600.0)),
        };
        let rect = super::physical_rect(bounds, 1.5);
        assert_eq!(rect.x, 150);
        assert_eq!(rect.y, 75);
        assert_eq!(rect.width, 1200);
        assert_eq!(rect.height, 900);
    }
}
