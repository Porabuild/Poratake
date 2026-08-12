use std::sync::OnceLock;
use windows::core::{Interface, Result};
use windows::Win32::System::Com::CoIncrementMTAUsage;

static PROCESS_MTA: OnceLock<Result<()>> = OnceLock::new();

pub fn retain_process_mta() -> Result<()> {
    PROCESS_MTA
        .get_or_init(|| unsafe { CoIncrementMTAUsage() }.map(|_| ()))
        .clone()
}

pub struct MtaInterface<T: Interface> {
    interface: T,
}

unsafe impl<T: Interface> Send for MtaInterface<T> {}
unsafe impl<T: Interface> Sync for MtaInterface<T> {}

impl<T: Interface> MtaInterface<T> {
    pub fn new(interface: T) -> Self {
        Self { interface }
    }

    pub unsafe fn with<R>(&self, action: impl FnOnce(&T) -> R) -> R {
        action(&self.interface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::GUID;
    use windows::Win32::Media::MediaFoundation::{
        IMFSourceReader, MFCreateAttributes, MFStartup, MFSTARTUP_FULL, MF_VERSION,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    const TEST_KEY: GUID = GUID::from_u128(0x2f3d6a4c_0f1b_4d64_9a0e_6f5c1b2d3e40);

    #[test]
    fn retaining_the_process_apartment_is_idempotent() {
        assert!(retain_process_mta().is_ok());
        assert!(retain_process_mta().is_ok());
    }

    #[test]
    fn mta_interfaces_move_between_threads_without_a_marshaling_proxy() {
        fn assert_shareable<T: Send + Sync>() {}
        assert_shareable::<MtaInterface<IMFSourceReader>>();

        retain_process_mta().expect("process MTA");
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .expect("MTA apartment");
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.expect("Media Foundation");
        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 1) }.expect("attributes");
        let shared = MtaInterface::new(attributes.expect("attributes"));

        let read = std::thread::spawn(move || {
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                .ok()
                .expect("MTA apartment");
            let read = unsafe {
                shared.with(|attributes| {
                    attributes.SetUINT32(&TEST_KEY, 7)?;
                    attributes.GetUINT32(&TEST_KEY)
                })
            };
            drop(shared);
            unsafe { CoUninitialize() };
            read
        })
        .join()
        .expect("worker thread");

        assert_eq!(read.expect("attribute round-trip"), 7);
        unsafe { CoUninitialize() };
    }
}
