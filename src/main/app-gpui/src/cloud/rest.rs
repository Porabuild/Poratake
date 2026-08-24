//! Self-hosted REST upload — port of `src/main/cloud/rest-client.ts`.

use crate::config::schema::RestProviderConfig;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Port of `getByPath`: dot notation with `[n]` array indexes.
pub fn value_at_path<'a>(
    source: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let normalized = path.replace('[', ".").replace(']', "");
    let mut current = source;
    for segment in normalized.split('.').filter(|part| !part.is_empty()) {
        current = match current {
            serde_json::Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            serde_json::Value::Object(map) => map.get(segment)?,
            _ => return None,
        };
    }
    Some(current)
}

/// Port of `extractUrlFromResponse`.
pub fn extract_url(body: &str, config: &RestProviderConfig) -> anyhow::Result<String> {
    let trimmed = body.trim();

    if config.response_is_plain_text {
        if trimmed.is_empty() {
            anyhow::bail!("Response body is empty");
        }
        return Ok(trimmed.to_string());
    }

    if config.response_url_path.is_empty() {
        anyhow::bail!("Response URL path is not configured");
    }

    let parsed: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|_| anyhow::anyhow!("Response is not valid JSON"))?;
    let value = value_at_path(&parsed, &config.response_url_path)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "URL not found at path \"{}\" in response",
                config.response_url_path
            )
        })?;
    Ok(value.to_string())
}

/// Port of `buildMultipartStream`, returning the body and its content type.
pub fn multipart_body(
    boundary: &str,
    field_name: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> Vec<u8> {
    let header = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
    );
    let footer = format!("\r\n--{boundary}--\r\n");

    let mut body = Vec::with_capacity(header.len() + bytes.len() + footer.len());
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(footer.as_bytes());
    body
}

fn boundary() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!("----PoratakeBoundary{nanos:032x}")
}

pub fn upload(
    config: &RestProviderConfig,
    filename: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<String> {
    if config.url.trim().is_empty() {
        anyhow::bail!("the upload URL is not configured");
    }
    let field_name = if config.file_field_name.trim().is_empty() {
        "file"
    } else {
        config.file_field_name.trim()
    };

    let boundary = boundary();
    let body = multipart_body(&boundary, field_name, filename, content_type, &bytes);

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .new_agent();
    let mut request = agent.post(&config.url).header(
        "Content-Type",
        &format!("multipart/form-data; boundary={boundary}"),
    );
    for header in &config.headers {
        if header.key.trim().is_empty() {
            continue;
        }
        request = request.header(header.key.trim(), &header.value);
    }

    let mut response = request
        .send(&body[..])
        .map_err(|error| anyhow::anyhow!("upload failed: {error}"))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        anyhow::bail!(
            "Upload failed: {status} - {}",
            text.chars().take(200).collect::<String>()
        );
    }
    extract_url(&text, config)
}

pub fn probe(config: &RestProviderConfig) -> anyhow::Result<()> {
    if config.url.trim().is_empty() {
        anyhow::bail!("the upload URL is not configured");
    }
    if !config.response_is_plain_text && config.response_url_path.trim().is_empty() {
        anyhow::bail!("the response URL path is not configured");
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build()
        .new_agent();
    agent
        .head(&config.url)
        .call()
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("could not reach the endpoint: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> RestProviderConfig {
        RestProviderConfig::default()
    }

    #[test]
    fn reads_plain_text_responses_verbatim() {
        let mut config = config();
        config.response_is_plain_text = true;
        assert_eq!(
            extract_url("  https://cdn.example.com/a.png \n", &config).unwrap(),
            "https://cdn.example.com/a.png"
        );
        assert!(extract_url("   ", &config).is_err());
    }

    #[test]
    fn walks_dotted_json_paths_with_array_indexes() {
        let body = r#"{"data":{"files":[{"url":"https://x/y.png"}]}}"#;
        let mut config = config();
        config.response_url_path = "data.files[0].url".into();
        assert_eq!(extract_url(body, &config).unwrap(), "https://x/y.png");

        config.response_url_path = "data.missing".into();
        assert!(extract_url(body, &config).is_err());
    }

    #[test]
    fn requires_a_configured_path_for_json_responses() {
        let config = config();
        assert!(extract_url("{}", &config)
            .unwrap_err()
            .to_string()
            .contains("Response URL path"));
    }

    #[test]
    fn multipart_body_wraps_the_payload() {
        let body = multipart_body("BOUND", "file", "a.png", "image/png", b"12345");
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(
            "--BOUND\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\""
        ));
        assert!(text.contains("Content-Type: image/png\r\n\r\n12345\r\n--BOUND--\r\n"));
    }
}
