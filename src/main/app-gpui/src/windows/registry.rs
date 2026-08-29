use std::collections::HashMap;

use gpui::{AnyWindowHandle, App, Global};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WindowKind {
    Settings,
    History,
    VideoEditor,
    Onboarding,
    RecordingControl,
    #[cfg(not(windows))]
    TrayMenu,
}

#[derive(Default)]
struct WindowRegistry {
    handles: HashMap<WindowKind, AnyWindowHandle>,
}

impl Global for WindowRegistry {}

fn registry(cx: &mut App) -> &mut WindowRegistry {
    if cx.try_global::<WindowRegistry>().is_none() {
        cx.set_global(WindowRegistry::default());
    }
    cx.global_mut::<WindowRegistry>()
}

pub fn activate(kind: WindowKind, cx: &mut App) -> bool {
    let Some(handle) = registry(cx).handles.get(&kind).copied() else {
        return false;
    };
    let activated = handle
        .update(cx, |_, window, _| window.activate_window())
        .is_ok();
    if !activated {
        registry(cx).handles.remove(&kind);
    }
    activated
}

pub fn open_or_activate(
    kind: WindowKind,
    cx: &mut App,
    open: impl FnOnce(&mut App) -> Option<AnyWindowHandle>,
) {
    if activate(kind, cx) {
        return;
    }
    if let Some(handle) = open(cx) {
        registry(cx).handles.insert(kind, handle);
    }
}

pub fn close(kind: WindowKind, cx: &mut App) {
    let Some(handle) = registry(cx).handles.remove(&kind) else {
        return;
    };
    let _ = handle.update(cx, |_, window, _| window.remove_window());
}

pub fn forget(kind: WindowKind, cx: &mut App) {
    registry(cx).handles.remove(&kind);
}

pub fn is_open(kind: WindowKind, cx: &mut App) -> bool {
    let Some(handle) = registry(cx).handles.get(&kind).copied() else {
        return false;
    };
    let alive = handle.update(cx, |_, _, _| ()).is_ok();
    if !alive {
        registry(cx).handles.remove(&kind);
    }
    alive
}

pub fn toggle(
    kind: WindowKind,
    cx: &mut App,
    open: impl FnOnce(&mut App) -> Option<AnyWindowHandle>,
) {
    if is_open(kind, cx) {
        close(kind, cx);
        return;
    }
    open_or_activate(kind, cx, open);
}
