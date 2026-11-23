use super::super::thread_future::ThreadFuture;
use super::super::utils::init_com;
use super::dialog_ffi::IDialog;

use crate::file_handle::FileHandle;

pub fn single_return_future<F: FnOnce() -> Result<IDialog, i32> + Send + 'static>(
    build: F,
) -> ThreadFuture<Option<FileHandle>> {
    ThreadFuture::new(move |data| {
        let ret: Result<(), i32> = (|| {
            init_com(|| {
                let dialog = build()?;
                dialog.show()?;

                let path = dialog.get_result().ok().map(FileHandle::wrap);
                *data = Some(path);

                Ok(())
            })?
        })();

        if let Err(hr) = ret {
            eprintln!("[diag-rfd] dialog failed hr=0x{:08x}", hr as u32);
            *data = Some(None);
        }
    })
}

pub fn multiple_return_future<F: FnOnce() -> Result<IDialog, i32> + Send + 'static>(
    build: F,
) -> ThreadFuture<Option<Vec<FileHandle>>> {
    ThreadFuture::new(move |data| {
        let ret: Result<(), i32> = (|| {
            init_com(|| {
                let dialog = build()?;
                dialog.show()?;

                let list = dialog
                    .get_results()
                    .ok()
                    .map(|r| r.into_iter().map(FileHandle::wrap).collect());
                *data = Some(list);

                Ok(())
            })?
        })();

        if let Err(hr) = ret {
            eprintln!("[diag-rfd] dialog failed hr=0x{:08x}", hr as u32);
            *data = Some(None);
        }
    })
}
