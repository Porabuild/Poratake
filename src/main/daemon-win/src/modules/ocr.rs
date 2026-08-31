use crate::com::retain_process_mta;
use crate::protocol::{Request, params as parse_params, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use poratake_daemon_common::contract::{OCR_MODULE, OcrMethod, OcrRecognizeRequest};
use serde_json::json;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
    ExifOrientationMode,
};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::{FileAccessMode, StorageFile};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::HSTRING;

const OCR_SCALE_FACTOR: u32 = 2;

pub struct OcrModule;

impl Module for OcrModule {
    fn name(&self) -> &'static str {
        OCR_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match OcrMethod::parse(&request.method) {
            Some(OcrMethod::Recognize) => self.recognize(request),
            None => method_not_found(&request.method),
        }
    }
}

impl OcrModule {
    fn recognize(&self, request: &Request) -> Reply {
        let params: OcrRecognizeRequest = match parse_params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };
        if !params.image_path.is_file() {
            return Reply::Now(Err((
                "FILE_NOT_FOUND".to_string(),
                format!("Image file not found: {}", params.image_path.display()),
            )));
        }

        let request_id = request.id.clone();
        let image_path = params.image_path.to_string_lossy().into_owned();
        let worker = std::thread::Builder::new()
            .name("ocr-recognition".to_string())
            .spawn(move || match recognize_text(&image_path) {
                Ok(text) => respond_success(&request_id, json!({ "text": text })),
                Err(error) => {
                    respond_error(&request_id, "OCR_FAILED", &format!("OCR failed: {error}"))
                }
            });

        match worker {
            Ok(_) => Reply::Deferred,
            Err(error) => Reply::Now(Err((
                "OCR_FAILED".to_string(),
                format!("Failed to start OCR: {error}"),
            ))),
        }
    }
}

fn recognize_text(image_path: &str) -> windows::core::Result<String> {
    let _apartment = OcrApartment::initialize()?;
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(image_path))?.join()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let width = decoder.PixelWidth()?;
    let height = decoder.PixelHeight()?;
    let maximum = OcrEngine::MaxImageDimension()?;
    let (scaled_width, scaled_height) = fit_ocr_dimensions(width, height, maximum);
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(scaled_width)?;
    transform.SetScaledHeight(scaled_height)?;
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::RespectExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?;

    let engine = create_ocr_engine()?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;

    let mut lines = Vec::new();
    for line in result.Lines()? {
        lines.push(line.Text()?.to_string());
    }

    Ok(lines.join("\n").trim().to_string())
}

fn create_ocr_engine() -> windows::core::Result<OcrEngine> {
    if let Ok(language_tag) = Language::CurrentInputMethodLanguageTag()
        && let Ok(language) = Language::CreateLanguage(&language_tag)
        && let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language)
    {
        return Ok(engine);
    }

    let profile_error = match OcrEngine::TryCreateFromUserProfileLanguages() {
        Ok(engine) => return Ok(engine),
        Err(error) => error,
    };
    let languages = match OcrEngine::AvailableRecognizerLanguages() {
        Ok(languages) => languages,
        Err(_) => return Err(profile_error),
    };
    let language_count = match languages.Size() {
        Ok(count) => count,
        Err(_) => return Err(profile_error),
    };

    for index in 0..language_count {
        let Ok(language) = languages.GetAt(index) else {
            continue;
        };
        let engine = OcrEngine::TryCreateFromLanguage(&language);
        if let Ok(engine) = engine {
            return Ok(engine);
        }
    }

    Err(profile_error)
}

fn fit_ocr_dimensions(width: u32, height: u32, maximum: u32) -> (u32, u32) {
    if maximum == 0 || width == 0 || height == 0 {
        return (width, height);
    }

    let longest_side = width.max(height);
    let scaled_longest_side = longest_side.saturating_mul(OCR_SCALE_FACTOR).min(maximum);

    if width >= height {
        let scaled_height =
            ((u64::from(height) * u64::from(scaled_longest_side)) / u64::from(width)).max(1) as u32;
        return (scaled_longest_side, scaled_height);
    }

    let scaled_width =
        ((u64::from(width) * u64::from(scaled_longest_side)) / u64::from(height)).max(1) as u32;
    (scaled_width, scaled_longest_side)
}

struct OcrApartment;

impl OcrApartment {
    fn initialize() -> windows::core::Result<Self> {
        retain_process_mta()?;
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
        Ok(Self)
    }
}

impl Drop for OcrApartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enlarges_small_images_for_text_recognition() {
        assert_eq!(fit_ocr_dimensions(1009, 730, 2600), (2018, 1460));
    }

    #[test]
    fn enlarges_images_only_up_to_the_ocr_limit() {
        assert_eq!(fit_ocr_dimensions(1920, 1080, 2600), (2600, 1462));
    }

    #[test]
    fn constrains_landscape_images_without_changing_aspect_ratio() {
        assert_eq!(fit_ocr_dimensions(7680, 4320, 2600), (2600, 1462));
    }

    #[test]
    fn constrains_portrait_images_without_changing_aspect_ratio() {
        assert_eq!(fit_ocr_dimensions(4320, 7680, 2600), (1462, 2600));
    }
}
