pub const SOURCE_URL: &str = "https://github.com/Porabuild/Poratake";
pub const ISSUES_URL: &str = "https://github.com/Porabuild/Poratake/issues";
pub const PORABUILD_URL: &str = "https://porabuild.com";
pub const PRODUCT_HOMEPAGE: &str = "https://porabuild.com/poratake";
pub const UPSTREAM_URL: &str = "https://github.com/capty-app/capty";
pub const VERSION: &str = env!("PORATAKE_PACKAGE_VERSION");

pub fn source_url_for_version() -> String {
    format!("{SOURCE_URL}/tree/v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn read_package_json_version() -> String {
        let package_json = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../package.json");
        let contents = std::fs::read_to_string(&package_json)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", package_json.display()));
        let key_pos = contents
            .find("\"version\"")
            .unwrap_or_else(|| panic!("no \"version\" field found in {}", package_json.display()));
        let after_key = &contents[key_pos + "\"version\"".len()..];
        let colon_pos = after_key
            .find(':')
            .unwrap_or_else(|| panic!("malformed \"version\" field in {}", package_json.display()));
        let after_colon = &after_key[colon_pos + 1..];
        let quote_start = after_colon
            .find('"')
            .unwrap_or_else(|| panic!("malformed \"version\" value in {}", package_json.display()));
        let value_start = quote_start + 1;
        let quote_end = after_colon[value_start..].find('"').unwrap_or_else(|| {
            panic!(
                "unterminated \"version\" value in {}",
                package_json.display()
            )
        });
        after_colon[value_start..value_start + quote_end].to_string()
    }

    fn read_cargo_toml_version() -> String {
        let cargo_toml = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let contents = std::fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", cargo_toml.display()));
        let line = contents
            .lines()
            .find(|line| line.trim_start().starts_with("version"))
            .unwrap_or_else(|| panic!("no version field found in {}", cargo_toml.display()));
        let quote_start = line
            .find('"')
            .unwrap_or_else(|| panic!("malformed version field in {}", cargo_toml.display()));
        let value_start = quote_start + 1;
        let quote_end = line[value_start..]
            .find('"')
            .unwrap_or_else(|| panic!("unterminated version value in {}", cargo_toml.display()));
        line[value_start..value_start + quote_end].to_string()
    }

    #[test]
    fn version_matches_package_json() {
        let expected = read_package_json_version();
        assert_eq!(
            read_cargo_toml_version(),
            expected,
            "Cargo.toml version has drifted from package.json"
        );
        assert_eq!(VERSION, expected);
        assert!(source_url_for_version().ends_with(&format!("/tree/v{expected}")));
    }
}
