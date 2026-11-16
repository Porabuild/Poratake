use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use herogpui::components::TooltipPresentationRequest;
use herogpui::gpui::Window;
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub struct MacTooltip {
    view: id,
}

impl MacTooltip {
    pub fn new(window: &Window) -> Option<Self> {
        let view = native_view(window)?;
        unsafe {
            let _: id = msg_send![view, retain];
        }
        Some(Self { view })
    }

    pub fn show(&mut self, _window: &Window, request: &TooltipPresentationRequest) {
        unsafe { set_tooltip(self.view, Some(&request.content)) };
    }

    pub fn hide(&mut self) {
        unsafe { set_tooltip(self.view, None) };
    }
}

impl Drop for MacTooltip {
    fn drop(&mut self) {
        self.hide();
        unsafe {
            let _: () = msg_send![self.view, release];
        }
    }
}

pub fn set_native_tooltip(window: &Window, tooltip: Option<&str>) {
    if let Some(view) = native_view(window) {
        unsafe { set_tooltip(view, tooltip) };
    }
}

fn native_view(window: &Window) -> Option<id> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    Some(appkit.ns_view.as_ptr() as id)
}

unsafe fn set_tooltip(view: id, tooltip: Option<&str>) {
    let value = tooltip
        .map(|tooltip| NSString::alloc(nil).init_str(tooltip))
        .unwrap_or(nil);
    let _: () = msg_send![view, setToolTip: value];
    if value != nil {
        let _: () = msg_send![value, release];
    }
}
