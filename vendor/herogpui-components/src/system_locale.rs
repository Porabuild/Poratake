//! The operating system's date-and-time locale preference chain.
//!
//! Dates and times follow the platform's *regional format* setting, not its
//! interface language: a reader can run an English interface and still expect
//! German dates. Each platform reports that setting differently, so this reads
//! the one source per platform that carries it, most preferred tag first, as
//! BCP 47 strings. Callers try each tag in turn because CLDR may know a region
//! the platform reports but not the exact tag spelling.
//!
//! The POSIX environment is read first on **every** platform, as
//! `locale_config` did (many users set it everywhere, and it is how a
//! process or test pins a locale): `LC_ALL` alone when set; otherwise
//! `LC_TIME`, then `LANG`, with `LANGUAGE` entries as fallbacks. Only when it
//! names no locale does the platform's own setting answer:
//!
//! | platform | regional format | fallbacks |
//! |---|---|---|
//! | macOS | `NSLocale.currentLocale.localeIdentifier` (the Region setting) | the preferred languages |
//! | Windows | `HKCU\Control Panel\International\LocaleName` (the Regional format) | the preferred UI languages |
//! | Linux, other Unix, wasm32 | none beyond the environment | none |
//!
//! This replaced the `locale_config` crate, whose macOS backend pulled in
//! `objc` 0.2 and `objc-foundation` 0.1 and through them the `block` 0.1.6
//! future-incompatibility warning (retired with `gpui-pre` 0.3.11, whose
//! macOS graph no longer contains `block`).
//! The sources and their precedence are the ones `locale_config` read for its
//! `time` category (less its CGI `HTTP_ACCEPT_LANGUAGE` probe, meaningless
//! for a GUI), so the detected locale is unchanged. `sys-locale` alone is not
//! a replacement: it reports the *interface* languages (`LC_MESSAGES`,
//! `CFLocaleCopyPreferredLanguages`, `GetUserPreferredUILanguages`), which is
//! why it only supplies the fallbacks here.

/// The locale tags the system prefers for dates and times, most preferred
/// first, deduplicated. Empty when the platform reports nothing usable.
pub fn time_locale_tags() -> Vec<String> {
    let mut tags = Vec::new();
    for tag in platform_tags() {
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

fn platform_tags() -> Vec<String> {
    let environment = unix_time_tags(|name| std::env::var(name).ok());
    if environment.is_empty() {
        native_tags()
    } else {
        environment
    }
}

#[cfg(target_os = "macos")]
fn native_tags() -> Vec<String> {
    let region = objc2_foundation::NSLocale::currentLocale()
        .localeIdentifier()
        .to_string();
    posix_to_bcp47(&region)
        .into_iter()
        .chain(sys_locale::get_locales())
        .collect()
}

#[cfg(windows)]
fn native_tags() -> Vec<String> {
    windows_registry::CURRENT_USER
        .open(r"Control Panel\International")
        .and_then(|key| key.get_string("LocaleName"))
        .ok()
        .into_iter()
        .chain(sys_locale::get_locales())
        .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn native_tags() -> Vec<String> {
    Vec::new()
}

/// The POSIX environment's `time` chain, with `var` standing in for
/// `std::env::var` so it can be tested without mutating the process
/// environment. `LC_ALL` overrides every category; otherwise `LC_TIME` wins
/// over the `LANG` default, and `LANGUAGE` (colon-separated) adds fallbacks.
fn unix_time_tags(var: impl Fn(&str) -> Option<String>) -> Vec<String> {
    if let Some(tag) = var("LC_ALL").and_then(|value| posix_to_bcp47(&value)) {
        return vec![tag];
    }
    let mut tags: Vec<String> = ["LC_TIME", "LANG"]
        .into_iter()
        .filter_map(|name| var(name).and_then(|value| posix_to_bcp47(&value)))
        .collect();
    if let Some(languages) = var("LANGUAGE") {
        tags.extend(languages.split(':').filter_map(posix_to_bcp47));
    }
    tags
}

/// `de_DE.UTF-8@euro` -> `de-DE`, `sr_RS@latin` -> `sr-Latn-RS`,
/// `en_US@rg=dezzzz` (macOS's region override) -> `en-US-u-rg-dezzzz`.
/// `C`, `POSIX` and empty values carry no locale and yield `None`; a value
/// that is already BCP 47 passes through.
fn posix_to_bcp47(value: &str) -> Option<String> {
    let value = value.trim();
    let (base, modifier) = value.split_once('@').unwrap_or((value, ""));
    let base = base.split_once('.').map_or(base, |(base, _codeset)| base);
    if base.is_empty() || base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
        return None;
    }
    let mut parts = base.split(['_', '-']);
    let language = parts.next()?.to_ascii_lowercase();
    if !(2..=8).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let rest: Vec<&str> = parts.collect();
    let script = match modifier.to_ascii_lowercase().as_str() {
        "latin" | "latn" | "iqtelif" | "ijekavianlatin" => Some("Latn"),
        "cyrillic" | "cyrl" => Some("Cyrl"),
        "arabic" => Some("Arab"),
        "devanagari" => Some("Deva"),
        "hebrew" => Some("Hebr"),
        _ => None,
    };
    let mut tag = language;
    if let Some(script) = script {
        tag.push('-');
        tag.push_str(script);
    }
    for part in rest {
        tag.push('-');
        tag.push_str(part);
    }
    if let Some(region) = modifier.strip_prefix("rg=") {
        tag.push_str("-u-rg-");
        tag.push_str(&region.to_ascii_lowercase());
    }
    Some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn posix_names_become_bcp47_tags() {
        assert_eq!(posix_to_bcp47("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(posix_to_bcp47("de_DE.UTF-8@euro").as_deref(), Some("de-DE"));
        assert_eq!(posix_to_bcp47("sr_RS@latin").as_deref(), Some("sr-Latn-RS"));
        assert_eq!(
            posix_to_bcp47("en_US@rg=dezzzz").as_deref(),
            Some("en-US-u-rg-dezzzz")
        );
        assert_eq!(posix_to_bcp47("en-GB").as_deref(), Some("en-GB"));
        assert_eq!(posix_to_bcp47("ja").as_deref(), Some("ja"));
        assert_eq!(posix_to_bcp47("C"), None);
        assert_eq!(posix_to_bcp47("C.UTF-8"), None);
        assert_eq!(posix_to_bcp47("POSIX"), None);
        assert_eq!(posix_to_bcp47(""), None);
    }

    #[test]
    fn lc_time_wins_over_lang() {
        let tags = unix_time_tags(env(&[("LANG", "en_US.UTF-8"), ("LC_TIME", "de_DE.UTF-8")]));
        assert_eq!(tags, ["de-DE", "en-US"]);
    }

    #[test]
    fn lc_all_overrides_every_category() {
        let tags = unix_time_tags(env(&[
            ("LC_ALL", "fr_FR.UTF-8"),
            ("LC_TIME", "de_DE.UTF-8"),
            ("LANG", "en_US.UTF-8"),
        ]));
        assert_eq!(tags, ["fr-FR"]);
    }

    #[test]
    fn language_adds_fallbacks_and_c_is_skipped() {
        let tags = unix_time_tags(env(&[("LANG", "C"), ("LANGUAGE", "pt_BR:pt::en")]));
        assert_eq!(tags, ["pt-BR", "pt", "en"]);
    }

    #[test]
    fn an_unusable_lc_all_falls_through_to_the_categories() {
        let tags = unix_time_tags(env(&[("LC_ALL", "C"), ("LANG", "ko_KR.UTF-8")]));
        assert_eq!(tags, ["ko-KR"]);
    }
}
