use crate::protocol::{param_str, Request};
use crate::router::{method_not_found, Module, Reply};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use windows::core::HSTRING;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::{FileAccessMode, StorageFile};

pub struct OcrModule;

impl Module for OcrModule {
    fn name(&self) -> &'static str {
        "ocr"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "recognize" => self.recognize(&request.params).into(),
            method => method_not_found(method),
        }
    }
}

impl OcrModule {
    fn recognize(
        &self,
        params: &Option<HashMap<String, Value>>,
    ) -> Result<Option<Value>, (String, String)> {
        let Some(image_path) = param_str(params, "imagePath") else {
            return Err((
                "INVALID_PARAMS".to_string(),
                "Missing imagePath parameter".to_string(),
            ));
        };

        if !Path::new(image_path).exists() {
            return Err((
                "FILE_NOT_FOUND".to_string(),
                format!("Image file not found: {image_path}"),
            ));
        }

        let text = recognize_text(image_path)
            .map_err(|error| ("OCR_FAILED".to_string(), format!("OCR failed: {error}")))?;

        Ok(Some(json!({ "text": text })))
    }
}

fn recognize_text(image_path: &str) -> windows::core::Result<String> {
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(image_path))?.join()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let bitmap = decoder.GetSoftwareBitmapAsync()?.join()?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;

    let mut lines = Vec::new();
    for line in result.Lines()? {
        lines.push(line.Text()?.to_string());
    }

    Ok(lines.join("\n").trim().to_string())
}
