use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::config::store::config_dir;
use crate::history_store::HistoryItemType;

const THUMBNAIL_WIDTH: u32 = 300;
const THUMBNAIL_QUALITY: u8 = 80;

pub fn thumbnails_dir() -> PathBuf {
    config_dir().join("thumbnails")
}

pub fn thumbnail_path(original: &Path) -> PathBuf {
    let mut hasher = Md5::new();
    hasher.update(original.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    thumbnails_dir().join(format!("{hex}.jpg"))
}

pub(crate) fn screenshot_thumbnail_path(original: &Path) -> PathBuf {
    thumbnail_path(original).with_extension("srgb.jpg")
}

pub fn cached(original: &Path) -> Option<PathBuf> {
    let path = thumbnail_path(original);
    path.is_file().then_some(path)
}

pub fn ensure(original: &Path, kind: HistoryItemType) -> Option<PathBuf> {
    if kind == HistoryItemType::Video {
        return cached(original);
    }
    let target = screenshot_thumbnail_path(original);
    if target.is_file() {
        return Some(target);
    }
    if original.is_file() && generate(original, &target) {
        return Some(target);
    }
    cached(original)
}

#[cfg(target_os = "macos")]
fn generate_native_thumbnail(original: &Path, target: &Path) -> Option<Vec<u8>> {
    use std::ffi::{c_char, c_void, CString};

    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const CF_NUMBER_SINT32: isize = 3;
    const CF_NUMBER_FLOAT64: isize = 6;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: *const c_void;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: *const c_void,
            buffer: *const u8,
            buf_len: isize,
            is_directory: u8,
        ) -> *const c_void;
        fn CFNumberCreate(
            allocator: *const c_void,
            the_type: isize,
            value_ptr: *const c_void,
        ) -> *const c_void;
        fn CFDataCreateMutable(allocator: *const c_void, capacity: isize) -> *mut c_void;
        fn CFDataGetLength(the_data: *const c_void) -> isize;
        fn CFDataGetBytePtr(the_data: *const c_void) -> *const u8;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        static kCGImageSourceThumbnailMaxPixelSize: *const c_void;
        static kCGImageSourceCreateThumbnailFromImageAlways: *const c_void;
        static kCGImageSourceCreateThumbnailWithTransform: *const c_void;
        static kCGImageDestinationLossyCompressionQuality: *const c_void;
        fn CGImageSourceCreateWithURL(url: *const c_void, options: *const c_void) -> *const c_void;
        fn CGImageSourceCreateThumbnailAtIndex(
            isrc: *const c_void,
            index: usize,
            options: *const c_void,
        ) -> *const c_void;
        fn CGImageDestinationCreateWithData(
            data: *mut c_void,
            type_id: *const c_void,
            count: usize,
            options: *const c_void,
        ) -> *mut c_void;
        fn CGImageDestinationAddImage(
            idst: *mut c_void,
            image: *const c_void,
            properties: *const c_void,
        );
        fn CGImageDestinationFinalize(idst: *mut c_void) -> u8;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageRelease(image: *const c_void);
    }

    struct CfRef(*const c_void);
    impl Drop for CfRef {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    let _ = target;
    let raw_path = original.to_str()?;
    let bytes_path = raw_path.as_bytes();
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes_path.as_ptr(),
            bytes_path.len() as isize,
            0,
        );
        if url.is_null() {
            return None;
        }
        let _url = CfRef(url);

        let source = CGImageSourceCreateWithURL(url, std::ptr::null());
        if source.is_null() {
            return None;
        }
        let _source = CfRef(source);

        let max_px = i32::try_from(THUMBNAIL_WIDTH).ok()?;
        let max_px_cf = CFNumberCreate(
            std::ptr::null(),
            CF_NUMBER_SINT32,
            (&max_px as *const i32).cast(),
        );
        if max_px_cf.is_null() {
            return None;
        }
        let _max_px_cf = CfRef(max_px_cf);

        let thumb_keys = [
            kCGImageSourceThumbnailMaxPixelSize,
            kCGImageSourceCreateThumbnailFromImageAlways,
            kCGImageSourceCreateThumbnailWithTransform,
        ];
        let thumb_values = [max_px_cf, kCFBooleanTrue, kCFBooleanTrue];
        let thumb_opts = CFDictionaryCreate(
            std::ptr::null(),
            thumb_keys.as_ptr(),
            thumb_values.as_ptr(),
            thumb_keys.len() as isize,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );
        if thumb_opts.is_null() {
            return None;
        }
        let _thumb_opts = CfRef(thumb_opts);

        let image = CGImageSourceCreateThumbnailAtIndex(source, 0, thumb_opts);
        if image.is_null() {
            return None;
        }
        struct ImageRef(*const c_void);
        impl Drop for ImageRef {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { CGImageRelease(self.0) };
                }
            }
        }
        let _image = ImageRef(image);

        let jpeg_uti_c = CString::new("public.jpeg").ok()?;
        let jpeg_uti = CFStringCreateWithCString(
            std::ptr::null(),
            jpeg_uti_c.as_ptr(),
            CF_STRING_ENCODING_UTF8,
        );
        if jpeg_uti.is_null() {
            return None;
        }
        let _jpeg_uti = CfRef(jpeg_uti);

        let out_data = CFDataCreateMutable(std::ptr::null(), 0);
        if out_data.is_null() {
            return None;
        }
        let _out_data = CfRef(out_data.cast());

        let dest = CGImageDestinationCreateWithData(out_data, jpeg_uti, 1, std::ptr::null());
        if dest.is_null() {
            return None;
        }
        let _dest = CfRef(dest.cast());

        let quality = f64::from(THUMBNAIL_QUALITY) / 100.0;
        let quality_cf = CFNumberCreate(
            std::ptr::null(),
            CF_NUMBER_FLOAT64,
            (&quality as *const f64).cast(),
        );
        if quality_cf.is_null() {
            return None;
        }
        let _quality_cf = CfRef(quality_cf);

        let dest_keys = [kCGImageDestinationLossyCompressionQuality];
        let dest_values = [quality_cf];
        let dest_opts = CFDictionaryCreate(
            std::ptr::null(),
            dest_keys.as_ptr(),
            dest_values.as_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );
        if dest_opts.is_null() {
            return None;
        }
        let _dest_opts = CfRef(dest_opts);

        CGImageDestinationAddImage(dest, image, dest_opts);
        if CGImageDestinationFinalize(dest) == 0 {
            return None;
        }
        let len = CFDataGetLength(out_data.cast());
        if len <= 0 {
            return None;
        }
        let ptr = CFDataGetBytePtr(out_data.cast());
        Some(std::slice::from_raw_parts(ptr, len as usize).to_vec())
    }
}

fn generate(original: &Path, target: &Path) -> bool {
    let Some(parent) = target.parent() else {
        return false;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        eprintln!(
            "[thumbnails] failed to create {}: {error}",
            parent.display()
        );
        return false;
    }

    #[cfg(target_os = "macos")]
    if let Some(bytes) = generate_native_thumbnail(original, target) {
        if let Err(error) = crate::util::atomic_file::write(target, &bytes) {
            eprintln!("[thumbnails] failed to save {}: {error}", target.display());
            return false;
        }
        return true;
    }

    let decoded = match crate::render::images::open(original) {
        Ok(decoded) => decoded,
        Err(error) => {
            eprintln!(
                "[thumbnails] failed to read {}: {error}",
                original.display()
            );
            return false;
        }
    };
    let resized = decoded
        .thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_WIDTH)
        .to_rgb8();

    let mut bytes = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, THUMBNAIL_QUALITY);
    if let Err(error) = encoder.encode_image(&resized) {
        eprintln!(
            "[thumbnails] failed to encode {}: {error}",
            target.display()
        );
        return false;
    }
    if let Err(error) = crate::util::atomic_file::write(target, &bytes) {
        eprintln!("[thumbnails] failed to save {}: {error}", target.display());
        return false;
    }
    true
}

pub fn remove(original: &Path) {
    for path in [
        thumbnail_path(original),
        screenshot_thumbnail_path(original),
    ] {
        if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn rekey(old: &Path, new: &Path) {
    let old_thumbnail = thumbnail_path(old);
    if !old_thumbnail.is_file() {
        return;
    }
    let new_thumbnail = thumbnail_path(new);
    if let Some(parent) = new_thumbnail.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(old_thumbnail, new_thumbnail);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_thumbnails_by_md5_of_the_source_path() {
        let path = thumbnail_path(Path::new("/tmp/example.png"));
        let name = path.file_name().and_then(|value| value.to_str()).unwrap();
        assert_eq!(name.len(), 36);
        assert!(name.ends_with(".jpg"));
        assert_eq!(path, thumbnail_path(Path::new("/tmp/example.png")));
        assert_ne!(path, thumbnail_path(Path::new("/tmp/other.png")));
    }

    #[test]
    fn generates_bounded_jpeg_thumbnail_from_large_png() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.png");
        let target = dir.path().join("thumb.srgb.jpg");
        image::RgbaImage::from_pixel(1200, 800, image::Rgba([20, 120, 220, 255]))
            .save(&source)
            .expect("save source");
        assert!(generate(&source, &target));
        let thumb = image::open(&target).expect("open thumbnail");
        assert!(thumb.width() <= THUMBNAIL_WIDTH);
        assert!(thumb.height() <= THUMBNAIL_WIDTH);
    }
}
