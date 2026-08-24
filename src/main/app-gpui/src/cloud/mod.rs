//! Cloud upload — port of `src/main/cloud/`. Uploads go only to the user's own
//! S3-compatible storage or self-hosted REST endpoint; Poratake never calls
//! Capty infrastructure.

pub mod rest;
pub mod s3;
pub mod sigv4;

use std::path::Path;

use crate::config::schema::CloudConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UploadState {
    Idle,
    Uploading,
    Success,
    Error,
}

pub fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        _ => "image/png",
    }
}

pub fn is_configured(config: &CloudConfig) -> bool {
    config.enabled && provider_fields_complete(config)
}

/// `isActiveProviderConfigured` in `setting-item-renderer.tsx`: whether the
/// active provider has every field it needs, regardless of whether uploads are
/// switched on. The Test Connection button gates on this rather than on
/// [`is_configured`].
pub fn provider_fields_complete(config: &CloudConfig) -> bool {
    match config.active_provider.as_str() {
        "rest" => {
            !config.rest.url.is_empty()
                && (config.rest.response_is_plain_text || !config.rest.response_url_path.is_empty())
        }
        _ => {
            !config.s3.endpoint.is_empty()
                && !config.s3.bucket.is_empty()
                && !config.s3.access_key_id.is_empty()
                && !config.s3.secret_access_key.is_empty()
        }
    }
}

/// Uploads `path` through the configured provider and returns the public URL.
pub fn upload(config: &CloudConfig, path: &Path) -> anyhow::Result<String> {
    if !config.enabled {
        anyhow::bail!("cloud upload is turned off in settings");
    }
    let bytes = std::fs::read(path)?;
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("capture.png")
        .to_string();
    let content_type = content_type_for(path);

    match config.active_provider.as_str() {
        "s3" => s3::upload(&config.s3, &filename, content_type, bytes),
        _ => rest::upload(&config.rest, &filename, content_type, bytes),
    }
}

/// Verifies the configured provider is reachable, backing the settings page's
/// "Test connection" button.
pub fn test_connection(config: &CloudConfig) -> anyhow::Result<()> {
    match config.active_provider.as_str() {
        "s3" => s3::head_bucket(&config.s3),
        _ => rest::probe(&config.rest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_extensions_to_content_types() {
        assert_eq!(content_type_for(Path::new("a.png")), "image/png");
        assert_eq!(content_type_for(Path::new("a.JPG")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("a.mp4")), "video/mp4");
        assert_eq!(content_type_for(Path::new("a")), "image/png");
    }

    #[test]
    fn provider_fields_complete_ignores_the_enabled_switch() {
        let mut config = CloudConfig::default();
        config.active_provider = "rest".into();
        config.rest.url = "https://example.test/upload".into();
        config.rest.response_is_plain_text = true;
        assert!(provider_fields_complete(&config));
        assert!(!is_configured(&config), "uploads are still switched off");
        config.enabled = true;
        assert!(is_configured(&config));
    }

    #[test]
    fn is_configured_requires_enabled_provider() {
        let mut config = CloudConfig::default();
        assert!(!is_configured(&config));
        config.enabled = true;
        assert!(!is_configured(&config));
        config.s3.endpoint = "https://s3.example".into();
        config.s3.bucket = "bucket".into();
        config.s3.access_key_id = "key".into();
        config.s3.secret_access_key = "secret".into();
        assert!(is_configured(&config));
        config.active_provider = "rest".into();
        assert!(!is_configured(&config));
        config.rest.url = "https://upload.example".into();
        config.rest.response_is_plain_text = true;
        assert!(is_configured(&config));
    }

    #[test]
    fn refuses_to_upload_while_disabled() {
        let config = CloudConfig::default();
        let error = upload(&config, Path::new("nope.png")).unwrap_err();
        assert!(error.to_string().contains("turned off"));
    }
}
