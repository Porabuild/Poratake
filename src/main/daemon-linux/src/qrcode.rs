use image::{DynamicImage, ImageDecoder as _, ImageReader, Limits};
use poratake_daemon_common::contract::{QRCODE_MODULE, QrCodeMethod};
use poratake_daemon_common::protocol::Request;
use poratake_daemon_common::qrcode::{
    MAX_QR_IMAGE_PIXELS, QrDetectionGate, detect_payload_from_greyscale, image_buffer_size,
    request_path, spawn_detection,
};
use poratake_daemon_common::router::{Module, Reply, method_not_found};

pub struct QrCodeModule {
    gate: QrDetectionGate,
}

impl QrCodeModule {
    pub fn new() -> Self {
        Self {
            gate: QrDetectionGate::default(),
        }
    }
}

impl Module for QrCodeModule {
    fn name(&self) -> &'static str {
        QRCODE_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match QrCodeMethod::parse(&request.method) {
            Some(QrCodeMethod::Detect) => {
                let image_path = match request_path(request) {
                    Ok(path) => path,
                    Err(error) => return Reply::Now(Err(error)),
                };
                spawn_detection(&self.gate, request.id.clone(), image_path, detect_qr_code)
            }
            None => method_not_found(&request.method),
        }
    }
}

fn detect_qr_code(image_path: &std::path::Path) -> Result<String, String> {
    let mut reader = ImageReader::open(image_path)
        .map_err(|error| format!("Failed to open image data: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Failed to inspect image data: {error}"))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_QR_IMAGE_PIXELS * 4);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|error| format!("Failed to decode image data: {error}"))?;
    let (width, height) = decoder.dimensions();
    image_buffer_size(width, height).map_err(str::to_string)?;
    let image = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Failed to decode image data: {error}"))?;
    let image = image.into_luma8();
    Ok(detect_payload_from_greyscale(
        image.as_raw(),
        image.width() as usize,
        image.height() as usize,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_dimensions_before_decoding_pixels() {
        let path = std::env::temp_dir().join(format!(
            "poratake-qrcode-oversized-{}.png",
            std::process::id()
        ));
        image::GrayImage::new(1, 1)
            .save(&path)
            .expect("write source PNG");
        let mut png = std::fs::read(&path).expect("read source PNG");
        png[16..20].copy_from_slice(&8193u32.to_be_bytes());
        png[20..24].copy_from_slice(&8192u32.to_be_bytes());
        let checksum = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&checksum.to_be_bytes());
        std::fs::write(&path, png).expect("write oversized PNG header");

        assert_eq!(
            detect_qr_code(&path),
            Err("Image dimensions exceed the QR detection limit".into())
        );
        std::fs::remove_file(path).expect("remove oversized PNG");
    }

    #[test]
    fn rejects_unknown_methods() {
        let mut module = QrCodeModule::new();
        let request = Request {
            id: "unknown".into(),
            module: QRCODE_MODULE.into(),
            method: "unknown".into(),
            params: None,
        };
        let Reply::Now(result) = module.handle(&request) else {
            panic!("unknown method should respond immediately");
        };
        assert_eq!(result.expect_err("unknown method").0, "METHOD_NOT_FOUND");
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut value = 0xffff_ffffu32;
        for &byte in data {
            value ^= u32::from(byte);
            for _ in 0..8 {
                let mask = if value & 1 != 0 { 0xedb8_8320 } else { 0 };
                value = (value >> 1) ^ mask;
            }
        }
        !value
    }
}
