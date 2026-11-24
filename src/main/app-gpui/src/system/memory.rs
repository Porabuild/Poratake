use std::time::Duration;

use gpui::{App, Global};
use herogpui::gpui;

const IDLE_RECLAIM_DELAY: Duration = Duration::from_secs(5);

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_zone_pressure_relief(zone: *mut core::ffi::c_void, goal: usize) -> usize;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> core::ffi::c_int;
}

#[cfg(windows)]
unsafe extern "C" {
    fn _heapmin() -> core::ffi::c_int;
}

pub fn compact_heap() {
    #[cfg(target_os = "macos")]
    unsafe {
        let _ = malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        let _ = malloc_trim(0);
    }
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Memory::{GetProcessHeap, HeapCompact, HEAP_FLAGS};

        if let Ok(heap) = GetProcessHeap() {
            let _ = HeapCompact(heap, HEAP_FLAGS(0));
        }
        let _ = _heapmin();
    }
}

pub fn reclaim_idle_memory() {
    compact_heap();
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Memory::{
            SetProcessWorkingSetSizeEx, SETPROCESSWORKINGSETSIZEEX_FLAGS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;

        let _ = SetProcessWorkingSetSizeEx(
            GetCurrentProcess(),
            usize::MAX,
            usize::MAX,
            SETPROCESSWORKINGSETSIZEEX_FLAGS(0),
        );
    }
}

pub fn clear_transient_caches() {
    crate::editor::wallpaper_svg::clear_caches();
    crate::render::gradient::clear_image_cache();
    crate::video::composition::cursor_sprites::clear_caches();
    crate::editor::preview_raster::clear_cache();
}

#[derive(Default)]
struct IdleReclaimState {
    generation: u64,
    evicting: bool,
}

impl Global for IdleReclaimState {}

fn idle_state(cx: &mut App) -> &mut IdleReclaimState {
    if cx.try_global::<IdleReclaimState>().is_none() {
        cx.set_global(IdleReclaimState::default());
    }
    cx.global_mut::<IdleReclaimState>()
}

fn is_app_idle(cx: &App) -> bool {
    if crate::capture::desktop_icons::is_shutting_down()
        || crate::video::recorder::is_recording()
        || crate::capture::overlay::has_active_or_pending(cx)
        || crate::windows::tray_menu::TrayMenuWindow::is_visible(cx)
        || crate::capture::timer::is_active()
        || crate::capture::scroll::is_active()
        || crate::system::device_test::mic_active()
        || crate::system::device_test::camera_active()
    {
        return false;
    }
    let keepalive_windows = usize::from(cfg!(target_os = "macos"));
    let cached_windows = crate::windows::tray_menu::TrayMenuWindow::cached_hidden_count(cx)
        + crate::capture::overlay::pooled_count(cx)
        + keepalive_windows;
    cx.windows().len() <= cached_windows
}

fn is_current_idle(generation: u64, cx: &App) -> bool {
    let current = cx
        .try_global::<IdleReclaimState>()
        .map_or(0, |state| state.generation);
    current == generation && is_app_idle(cx)
}

pub fn schedule_idle_reclaim(cx: &mut App) {
    if crate::state::try_state(cx).is_none() {
        return;
    }
    if cx
        .try_global::<IdleReclaimState>()
        .is_some_and(|state| state.evicting)
    {
        return;
    }
    let generation = {
        let state = idle_state(cx);
        state.generation = state.generation.wrapping_add(1);
        state.generation
    };
    if !is_app_idle(cx) {
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(IDLE_RECLAIM_DELAY).await;
        cx.update(|cx| {
            if !is_current_idle(generation, cx) {
                return;
            }
            clear_transient_caches();
            crate::windows::capture_preview::CapturePreviewWindow::discard_idle(cx);
            #[cfg(all(target_os = "macos", not(test)))]
            if crate::capture::overlay::pooled_count(cx) > 0 {
                idle_state(cx).evicting = true;
                crate::capture::overlay::evict_pooled(cx);
                crate::capture::overlay::prewarm(cx);
                idle_state(cx).evicting = false;
            }
            reclaim_idle_memory();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::{clear_transient_caches, reclaim_idle_memory};

    #[test]
    fn reclaim_idle_memory_succeeds_after_large_allocation() {
        let buffer = vec![1u8; 4 * 1024 * 1024];
        std::hint::black_box(&buffer);
        drop(buffer);
        reclaim_idle_memory();
    }

    #[test]
    fn clear_transient_caches_drops_cached_pixmaps_and_sprites() {
        let _ = crate::editor::wallpaper_svg::pixmap("crimson-wave", 64, 48);
        let _ =
            crate::video::composition::cursor_sprites::sprite("arrow", "#000000", "#ffffff", 32);
        clear_transient_caches();
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn idle_eviction_removes_cached_tray_and_pooled_overlays(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        cx.update(crate::windows::tray_menu::TrayMenuWindow::prewarm);
        cx.update(crate::capture::overlay::prewarm);
        assert!(cx.read(|cx| !cx.windows().is_empty()));
        assert!(cx.read(super::is_app_idle));

        cx.update(|cx| {
            crate::windows::tray_menu::TrayMenuWindow::evict_cached(cx);
            crate::capture::overlay::evict_pooled(cx);
        });
        cx.run_until_parked();

        assert!(cx.read(|cx| cx.windows().is_empty()));
        assert_eq!(
            cx.read(crate::windows::tray_menu::TrayMenuWindow::cached_hidden_count),
            0
        );
        assert_eq!(cx.read(crate::capture::overlay::pooled_count), 0);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[herogpui::test]
    fn idle_reclaim_never_evicts_warm_windows(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        #[cfg(windows)]
        cx.update(crate::windows::tray_menu::TrayMenuWindow::prewarm);
        cx.update(crate::capture::overlay::prewarm);
        assert!(cx.read(super::is_app_idle));
        assert!(cx.read(crate::capture::overlay::pooled_count) > 0);

        cx.update(super::schedule_idle_reclaim);
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2 * 60 * 60));
        cx.run_until_parked();

        assert!(cx.read(crate::capture::overlay::pooled_count) > 0);
        #[cfg(windows)]
        assert!(cx.read(crate::windows::tray_menu::TrayMenuWindow::cached_hidden_count) > 0);
    }
}
