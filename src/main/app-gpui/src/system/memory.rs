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

pub fn reclaim_idle_memory() {
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
        let _ = _heapmin();
    }
}

#[cfg(test)]
mod tests {
    use super::reclaim_idle_memory;

    #[test]
    fn reclaim_idle_memory_succeeds_after_large_allocation() {
        let buffer = vec![1u8; 4 * 1024 * 1024];
        std::hint::black_box(&buffer);
        drop(buffer);
        reclaim_idle_memory();
    }
}
