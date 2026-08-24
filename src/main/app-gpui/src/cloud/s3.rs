//! S3-compatible upload — port of `src/main/cloud/s3-client.ts` and
//! `buildS3PublicUrl` in `src/main/cloud/index.ts`.

use crate::cloud::sigv4::{self, SigningInput, EMPTY_PAYLOAD_HASH, UNSIGNED_PAYLOAD};
use crate::config::schema::S3ProviderConfig;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn host_of(endpoint: &str) -> &str {
    endpoint
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
}

fn base_url(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Port of `buildS3PublicUrl`.
pub fn public_url(config: &S3ProviderConfig, key: &str) -> String {
    if !config.custom_domain.trim().is_empty() {
        let domain = config.custom_domain.trim_end_matches('/');
        return format!("{domain}/{key}");
    }
    let host = host_of(&config.endpoint);
    if host.contains(&config.bucket) {
        format!("https://{host}/{key}")
    } else {
        format!("https://{host}/{}/{key}", config.bucket)
    }
}

pub fn object_key(config: &S3ProviderConfig, filename: &str) -> String {
    if config.path_prefix.trim().is_empty() {
        filename.to_string()
    } else {
        format!("{}{filename}", config.path_prefix)
    }
}

/// Percent-encodes a key the way S3 canonical URIs expect: every byte outside
/// the unreserved set, with `/` kept as a path separator.
pub fn encode_key(key: &str) -> String {
    key.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn missing_field(config: &S3ProviderConfig) -> Option<&'static str> {
    if config.endpoint.trim().is_empty() {
        return Some("endpoint");
    }
    if config.bucket.trim().is_empty() {
        return Some("bucket");
    }
    if config.access_key_id.trim().is_empty() {
        return Some("access key");
    }
    if config.secret_access_key.trim().is_empty() {
        return Some("secret key");
    }
    None
}

pub fn upload(
    config: &S3ProviderConfig,
    filename: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<String> {
    if let Some(field) = missing_field(config) {
        anyhow::bail!("the S3 {field} is not configured");
    }

    let key = object_key(config, filename);
    let canonical_uri = format!("/{}/{}", config.bucket, encode_key(&key));
    let amz_date = sigv4::amz_date(chrono::Utc::now());

    let signed = sigv4::sign(SigningInput {
        method: "PUT",
        host: host_of(&config.endpoint),
        canonical_uri: &canonical_uri,
        region: config.region.trim(),
        access_key_id: config.access_key_id.trim(),
        secret_access_key: config.secret_access_key.trim(),
        payload_hash: UNSIGNED_PAYLOAD,
        amz_date: &amz_date,
        extra_headers: &[
            ("content-type".to_string(), content_type.to_string()),
            ("x-amz-acl".to_string(), "public-read".to_string()),
        ],
    });

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .new_agent();
    let url = format!("{}{canonical_uri}", base_url(&config.endpoint));
    let mut request = agent.put(&url);
    for (name, value) in &signed.headers {
        if name == "host" {
            continue;
        }
        request = request.header(name, value);
    }

    let mut response = request
        .send(&bytes[..])
        .map_err(|error| anyhow::anyhow!("S3 upload failed: {error}"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let text = response.body_mut().read_to_string().unwrap_or_default();
        anyhow::bail!(
            "S3 upload failed: {status} - {}",
            text.chars().take(200).collect::<String>()
        );
    }

    Ok(public_url(config, &key))
}

pub fn head_bucket(config: &S3ProviderConfig) -> anyhow::Result<()> {
    if let Some(field) = missing_field(config) {
        anyhow::bail!("the S3 {field} is not configured");
    }

    let canonical_uri = format!("/{}", config.bucket);
    let amz_date = sigv4::amz_date(chrono::Utc::now());
    let signed = sigv4::sign(SigningInput {
        method: "HEAD",
        host: host_of(&config.endpoint),
        canonical_uri: &canonical_uri,
        region: config.region.trim(),
        access_key_id: config.access_key_id.trim(),
        secret_access_key: config.secret_access_key.trim(),
        payload_hash: EMPTY_PAYLOAD_HASH,
        amz_date: &amz_date,
        extra_headers: &[],
    });

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build()
        .new_agent();
    let url = format!("{}{canonical_uri}", base_url(&config.endpoint));
    let mut request = agent.head(&url);
    for (name, value) in &signed.headers {
        if name == "host" {
            continue;
        }
        request = request.header(name, value);
    }

    let response = request
        .call()
        .map_err(|error| anyhow::anyhow!("bucket check failed: {error}"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        anyhow::bail!("bucket check failed: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> S3ProviderConfig {
        S3ProviderConfig {
            endpoint: "s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: "shots".into(),
            access_key_id: "AKIA".into(),
            secret_access_key: "secret".into(),
            path_prefix: String::new(),
            custom_domain: String::new(),
        }
    }

    #[test]
    fn builds_public_urls_like_the_renderer() {
        let mut config = config();
        assert_eq!(
            public_url(&config, "a.png"),
            "https://s3.amazonaws.com/shots/a.png"
        );

        config.endpoint = "https://shots.s3.amazonaws.com".into();
        assert_eq!(
            public_url(&config, "a.png"),
            "https://shots.s3.amazonaws.com/a.png"
        );

        config.custom_domain = "https://cdn.example.com/".into();
        assert_eq!(
            public_url(&config, "a.png"),
            "https://cdn.example.com/a.png"
        );
    }

    #[test]
    fn prefixes_keys_when_configured() {
        let mut config = config();
        assert_eq!(object_key(&config, "a.png"), "a.png");
        config.path_prefix = "screenshots/".into();
        assert_eq!(object_key(&config, "a.png"), "screenshots/a.png");
    }

    #[test]
    fn percent_encodes_keys_but_keeps_separators() {
        assert_eq!(encode_key("shots/My Shot.png"), "shots/My%20Shot.png");
        assert_eq!(encode_key("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn reports_the_first_missing_credential() {
        let mut config = config();
        assert_eq!(missing_field(&config), None);
        config.bucket = String::new();
        assert_eq!(missing_field(&config), Some("bucket"));
        config.endpoint = String::new();
        assert_eq!(missing_field(&config), Some("endpoint"));
    }

    #[test]
    fn strips_the_scheme_from_the_signing_host() {
        assert_eq!(host_of("https://s3.example.com/"), "s3.example.com");
        assert_eq!(host_of("s3.example.com"), "s3.example.com");
        assert_eq!(base_url("s3.example.com"), "https://s3.example.com");
    }
}
