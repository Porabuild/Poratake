use crate::protocol::Request;
use crate::router::{Module, Reply, method_not_found};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use poratake_daemon_common::contract::{
    DESKTOP_WALLPAPER_MODULE, DesktopWallpaperMethod, DesktopWallpaperResult,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

pub struct DesktopWallpaperModule;

impl Module for DesktopWallpaperModule {
    fn name(&self) -> &'static str {
        DESKTOP_WALLPAPER_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match DesktopWallpaperMethod::parse(&request.method) {
            Some(DesktopWallpaperMethod::Get) => get_wallpaper().into(),
            None => method_not_found(&request.method),
        }
    }
}

fn get_wallpaper() -> Result<Option<Value>, (String, String)> {
    if let Some(path) = wallpaper_path_from_system() {
        return Ok(Some(
            serde_json::to_value(DesktopWallpaperResult::Path(path))
                .map_err(|error| ("WALLPAPER_UNAVAILABLE".into(), error.to_string()))?,
        ));
    }

    if let Some(data_url) = transcoded_wallpaper_data() {
        return Ok(Some(
            serde_json::to_value(DesktopWallpaperResult::Data(data_url))
                .map_err(|error| ("WALLPAPER_UNAVAILABLE".into(), error.to_string()))?,
        ));
    }

    Err((
        "WALLPAPER_UNAVAILABLE".to_string(),
        "Could not get desktop wallpaper".to_string(),
    ))
}

fn wallpaper_path_from_system() -> Option<String> {
    let mut buffer = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }

    let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if length == 0 {
        return None;
    }

    let path = String::from_utf16_lossy(&buffer[..length]);
    if Path::new(&path).is_file() {
        Some(path)
    } else {
        None
    }
}

fn transcoded_wallpaper_data() -> Option<String> {
    let app_data = std::env::var_os("APPDATA")?;
    let path = PathBuf::from(app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Themes")
        .join("TranscodedWallpaper");

    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }

    Some(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}
