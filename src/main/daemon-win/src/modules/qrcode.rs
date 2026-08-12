use crate::com::retain_process_mta;
use crate::protocol::{param_str, respond_error, respond_success, Request};
use crate::router::{method_not_found, Module, Reply};
use serde_json::json;
use std::path::Path;
use std::ptr::null;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat8bppGray, IWICImagingFactory, IWICPalette,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

pub struct QrCodeModule;

const MAX_QR_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;

impl Module for QrCodeModule {
    fn name(&self) -> &'static str {
        "qrcode"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "detect" => self.detect(request),
            method => method_not_found(method),
        }
    }
}

impl QrCodeModule {
    fn detect(&self, request: &Request) -> Reply {
        let Some(image_path) = param_str(&request.params, "imagePath") else {
            return Reply::Now(Err((
                "INVALID_PARAMS".to_string(),
                "Missing imagePath parameter".to_string(),
            )));
        };

        if !Path::new(image_path).exists() {
            return Reply::Now(Err((
                "FILE_NOT_FOUND".to_string(),
                format!("Image file not found: {image_path}"),
            )));
        }

        let request_id = request.id.clone();
        let image_path = image_path.to_string();
        let worker = std::thread::Builder::new()
            .name("qr-detection".to_string())
            .spawn(move || match detect_qr_code(&image_path) {
                Ok(payload) => respond_success(&request_id, json!({ "payload": payload })),
                Err(error) => respond_error(
                    &request_id,
                    "QR_DETECTION_FAILED",
                    &format!("QR code detection failed: {error}"),
                ),
            });

        match worker {
            Ok(_) => Reply::Deferred,
            Err(error) => Reply::Now(Err((
                "QR_DETECTION_FAILED".to_string(),
                format!("Failed to start QR code detection: {error}"),
            ))),
        }
    }
}

fn detect_qr_code(image_path: &str) -> Result<String, String> {
    let (pixels, width, height) = load_greyscale(image_path)?;
    Ok(detect_payload_from_greyscale(&pixels, width, height))
}

pub fn detect_payload_from_greyscale(pixels: &[u8], width: usize, height: usize) -> String {
    if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
        return String::new();
    }

    let mut image =
        rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| pixels[y * width + x]);
    let grids = image.detect_grids();

    for grid in grids {
        if let Ok((_meta, content)) = grid.decode() {
            if !content.is_empty() {
                return content;
            }
        }
    }

    String::new()
}

fn load_greyscale(image_path: &str) -> Result<(Vec<u8>, usize, usize), String> {
    let _apartment = QrApartment::initialize()?;

    load_greyscale_inner(image_path)
}

struct QrApartment;

impl QrApartment {
    fn initialize() -> Result<Self, String> {
        retain_process_mta()
            .map_err(|error| format!("Failed to retain the process COM apartment: {error}"))?;
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| format!("Failed to initialize COM: {error}"))?;
        Ok(Self)
    }
}

impl Drop for QrApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn load_greyscale_inner(image_path: &str) -> Result<(Vec<u8>, usize, usize), String> {
    let factory: IWICImagingFactory = unsafe {
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "Failed to create WIC factory".to_string())?
    };

    let wide_path: Vec<u16> = image_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let decoder = unsafe {
        factory
            .CreateDecoderFromFilename(
                PCWSTR(wide_path.as_ptr()),
                None,
                windows::Win32::Foundation::GENERIC_READ,
                WICDecodeMetadataCacheOnLoad,
            )
            .map_err(|_| "Failed to decode image data".to_string())?
    };

    let frame = unsafe {
        decoder
            .GetFrame(0)
            .map_err(|_| "Failed to decode image data".to_string())?
    };

    let mut width = 0u32;
    let mut height = 0u32;
    unsafe {
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|_| "Failed to decode image data".to_string())?;
    }

    if width == 0 || height == 0 {
        return Err("Failed to decode image data".to_string());
    }

    let stride = width;
    let buffer_size = qr_image_buffer_size(width, height)?;

    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(buffer_size)
        .map_err(|_| "Failed to decode image data".to_string())?;
    pixels.resize(buffer_size, 0);

    let converter = unsafe {
        factory
            .CreateFormatConverter()
            .map_err(|_| "Failed to decode image data".to_string())?
    };

    unsafe {
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat8bppGray,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|_| "Failed to decode image data".to_string())?;
        converter
            .CopyPixels(null(), stride, &mut pixels)
            .map_err(|_| "Failed to decode image data".to_string())?;
    }

    Ok((pixels, width as usize, height as usize))
}

fn qr_image_buffer_size(width: u32, height: u32) -> Result<usize, String> {
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_QR_IMAGE_PIXELS {
        return Err("Image dimensions exceed the QR detection limit".to_string());
    }

    usize::try_from(pixel_count).map_err(|_| "Failed to decode image data".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn detects_payload_from_known_greyscale_qr() {
        let pixels = std::fs::read(fixture_path("qr-hello.gray")).expect("fixture grayscale");
        let payload = detect_payload_from_greyscale(&pixels, 264, 264);
        assert_eq!(payload, "https://example.com/qr-test");
    }

    #[test]
    fn returns_empty_payload_when_no_code_present() {
        let white = vec![255u8; 64 * 64];
        let payload = detect_payload_from_greyscale(&white, 64, 64);
        assert_eq!(payload, "");
    }

    #[test]
    fn returns_empty_payload_for_invalid_dimensions() {
        let pixels = vec![0u8; 4];
        assert_eq!(detect_payload_from_greyscale(&pixels, 0, 0), "");
        assert_eq!(detect_payload_from_greyscale(&pixels, 10, 10), "");
    }

    #[test]
    fn rejects_images_above_the_detection_memory_limit() {
        assert_eq!(qr_image_buffer_size(8192, 8192), Ok(64 * 1024 * 1024));
        assert_eq!(
            qr_image_buffer_size(8193, 8192),
            Err("Image dimensions exceed the QR detection limit".to_string())
        );
    }

    #[test]
    fn detect_rejects_missing_image_path() {
        let module = QrCodeModule;
        let request = Request {
            id: "missing-path".to_string(),
            module: "qrcode".to_string(),
            method: "detect".to_string(),
            params: None,
        };
        let Reply::Now(result) = module.detect(&request) else {
            panic!("missing path should respond immediately");
        };
        let err = result.expect_err("missing path");
        assert_eq!(err.0, "INVALID_PARAMS");
    }

    #[test]
    fn detect_rejects_missing_file() {
        let module = QrCodeModule;
        let mut params = HashMap::new();
        params.insert(
            "imagePath".to_string(),
            json!("C:\\nonexistent\\poratake-qr-missing.png"),
        );
        let request = Request {
            id: "missing-file".to_string(),
            module: "qrcode".to_string(),
            method: "detect".to_string(),
            params: Some(params),
        };
        let Reply::Now(result) = module.detect(&request) else {
            panic!("missing file should respond immediately");
        };
        let err = result.expect_err("missing file should error");
        assert_eq!(err.0, "FILE_NOT_FOUND");
        assert!(err.1.contains("Image file not found"));
    }

    #[test]
    fn detect_returns_empty_payload_for_non_qr_image_bytes() {
        let path = std::env::temp_dir().join("poratake-qrcode-blank.png");
        write_minimal_png(&path, 32, 32, 255);
        let payload = detect_qr_code(&path.to_string_lossy()).expect("decode image");
        assert_eq!(payload, "");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detect_reports_invalid_image_data() {
        let path = std::env::temp_dir().join("poratake-qrcode-invalid.png");
        std::fs::write(&path, b"invalid image").expect("write invalid image");
        let result = detect_qr_code(&path.to_string_lossy());
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }

    fn write_minimal_png(path: &Path, width: u32, height: u32, gray: u8) {
        let mut raw = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            raw.extend_from_slice(&[gray, gray, gray, 255]);
        }

        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        write_chunk(&mut png, b"IHDR", &ihdr);

        let mut idat_raw = Vec::new();
        for row in 0..height {
            idat_raw.push(0);
            let start = (row * width * 4) as usize;
            let end = start + (width * 4) as usize;
            for px in raw[start..end].chunks_exact(4) {
                idat_raw.extend_from_slice(&px[..3]);
            }
        }
        let compressed = deflate_store(&idat_raw);
        write_chunk(&mut png, b"IDAT", &compressed);
        write_chunk(&mut png, b"IEND", &[]);

        std::fs::write(path, png).expect("write png");
    }

    fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        let mut hasher = crc32();
        hasher.update(chunk_type);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
    }

    fn deflate_store(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 16);
        out.push(0x78);
        out.push(0x01);

        let mut offset = 0usize;
        while offset < data.len() {
            let remaining = data.len() - offset;
            let block = remaining.min(65535);
            let is_final = offset + block >= data.len();
            out.push(if is_final { 0x01 } else { 0x00 });
            let len = block as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(&data[offset..offset + block]);
            offset += block;
        }

        let adler = adler32(data);
        out.extend_from_slice(&adler.to_be_bytes());
        out
    }

    fn adler32(data: &[u8]) -> u32 {
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        for byte in data {
            a = (a + *byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    struct Crc32 {
        value: u32,
    }

    fn crc32() -> Crc32 {
        Crc32 { value: 0xffff_ffff }
    }

    impl Crc32 {
        fn update(&mut self, data: &[u8]) {
            for &byte in data {
                self.value ^= u32::from(byte);
                for _ in 0..8 {
                    let mask = if self.value & 1 != 0 { 0xedb8_8320 } else { 0 };
                    self.value = (self.value >> 1) ^ mask;
                }
            }
        }

        fn finalize(self) -> u32 {
            !self.value
        }
    }
}
