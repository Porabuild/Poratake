#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_zone_pressure_relief(zone: *mut core::ffi::c_void, goal: usize) -> usize;
}

pub fn reclaim_idle_memory() {
    #[cfg(target_os = "macos")]
    unsafe {
        let _ = malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
    }
}
