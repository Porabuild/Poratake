//! Filename generation — port of `main/utils/filename-generator.ts` for the
//! default naming pattern tokens.

#[cfg(test)]
use chrono::TimeZone as _;
use chrono::{Datelike as _, Timelike as _};

const INVALID_FILENAME_CHARS: &[char] = &['/', '\\', '?', '*', ':', '"', '<', '>', '|'];

pub fn generate_filename(
    pattern: &str,
    type_label: &str,
    extension: &str,
    at: chrono::DateTime<chrono::Local>,
) -> String {
    let filename = pattern
        .replace("%type", type_label)
        .replace("%Y", &format!("{:04}", at.year()))
        .replace("%m", &format!("{:02}", at.month()))
        .replace("%d", &format!("{:02}", at.day()))
        .replace("%H", &format!("{:02}", at.hour()))
        .replace("%M", &format!("{:02}", at.minute()))
        .replace("%S", &format!("{:02}", at.second()));

    let sanitized = sanitize(&filename);
    format!("{sanitized}.{extension}")
}

fn sanitize(filename: &str) -> String {
    let cleaned: String = filename
        .chars()
        .map(|c| {
            if INVALID_FILENAME_CHARS.contains(&c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `generateScreenshotExportName` — the save dialog's default file name.
pub fn generate_screenshot_export_name(extension: &str) -> String {
    generate_filename(
        "%type %Y-%m-%d at %H.%M.%S",
        "Screenshot",
        extension,
        chrono::Local::now(),
    )
}

pub struct TokenInfo {
    pub token: &'static str,
    pub description: &'static str,
    pub example: String,
}

/// Port of `getAvailableTokens`.
pub fn available_tokens(at: chrono::DateTime<chrono::Local>) -> Vec<TokenInfo> {
    vec![
        TokenInfo {
            token: "%Y",
            description: "Full year",
            example: format!("{:04}", at.year()),
        },
        TokenInfo {
            token: "%m",
            description: "Month (01-12)",
            example: format!("{:02}", at.month()),
        },
        TokenInfo {
            token: "%d",
            description: "Day (01-31)",
            example: format!("{:02}", at.day()),
        },
        TokenInfo {
            token: "%H",
            description: "Hour (00-23)",
            example: format!("{:02}", at.hour()),
        },
        TokenInfo {
            token: "%M",
            description: "Minute (00-59)",
            example: format!("{:02}", at.minute()),
        },
        TokenInfo {
            token: "%S",
            description: "Second (00-59)",
            example: format!("{:02}", at.second()),
        },
        TokenInfo {
            token: "%type",
            description: "Capture type",
            example: "Screenshot".to_string(),
        },
    ]
}

/// Port of `validateNamingPattern`.
pub fn validate_naming_pattern(pattern: &str) -> Option<&'static str> {
    if pattern.trim().is_empty() {
        return Some("Pattern cannot be empty");
    }
    if pattern.chars().count() > 100 {
        return Some("Pattern is too long (max 100 characters)");
    }
    let mut without_tokens = pattern.replace("%type", "");
    for token in ["%Y", "%m", "%d", "%H", "%M", "%S"] {
        without_tokens = without_tokens.replace(token, "");
    }
    if without_tokens
        .chars()
        .any(|c| INVALID_FILENAME_CHARS.contains(&c))
    {
        return Some("Pattern contains invalid characters");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_naming_patterns_like_the_renderer() {
        assert_eq!(
            validate_naming_pattern("  "),
            Some("Pattern cannot be empty")
        );
        assert_eq!(
            validate_naming_pattern(&"a".repeat(101)),
            Some("Pattern is too long (max 100 characters)")
        );
        assert_eq!(
            validate_naming_pattern("shots/%Y"),
            Some("Pattern contains invalid characters")
        );
        assert_eq!(validate_naming_pattern("%type %Y-%m-%d at %H.%M.%S"), None);
    }

    #[test]
    fn expands_every_token() {
        let at = chrono::Local
            .with_ymd_and_hms(2026, 3, 7, 9, 5, 4)
            .single()
            .expect("timestamp");
        assert_eq!(
            generate_filename("%type %Y-%m-%d at %H.%M.%S", "Screenshot", "png", at),
            "Screenshot 2026-03-07 at 09.05.04.png"
        );
    }
}
