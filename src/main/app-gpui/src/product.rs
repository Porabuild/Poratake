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

    #[allow(dead_code)]
    #[path = "../../../../win_resource.rs"]
    mod win_resource;

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

    #[test]
    fn windows_resource_embeds_app_icon_and_version_metadata() {
        let icon_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../build/icon.ico");
        let ico_bytes = std::fs::read(&icon_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", icon_path.display()));
        let res = win_resource::build_poratake_res(
            "poratake-gpui",
            win_resource::PRODUCT_NAME,
            VERSION,
            &ico_bytes,
        );
        assert_eq!(res.len() % 4, 0);

        let mut cursor = 0usize;
        let mut entries = Vec::new();
        while cursor < res.len() {
            let data_size =
                u32::from_le_bytes(res[cursor..cursor + 4].try_into().unwrap()) as usize;
            let header_size =
                u32::from_le_bytes(res[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            assert_eq!(header_size, 32);
            let res_type = u16::from_le_bytes(res[cursor + 10..cursor + 12].try_into().unwrap());
            let res_id = u16::from_le_bytes(res[cursor + 14..cursor + 16].try_into().unwrap());
            let data_start = cursor + header_size;
            let data_end = data_start + data_size;
            entries.push((res_type, res_id, &res[data_start..data_end]));
            cursor = (data_end + 3) & !3;
        }

        assert_eq!(entries[0].0, 0);
        assert_eq!(entries[0].1, 0);
        let icon_entries: Vec<_> = entries.iter().filter(|(t, _, _)| *t == 3).collect();
        let group_entries: Vec<_> = entries.iter().filter(|(t, _, _)| *t == 14).collect();
        let version_entries: Vec<_> = entries.iter().filter(|(t, _, _)| *t == 16).collect();
        assert_eq!(icon_entries.len(), 6);
        assert_eq!(group_entries.len(), 1);
        assert_eq!(group_entries[0].1, 1);
        assert_eq!(version_entries.len(), 1);
        assert_eq!(version_entries[0].1, 1);

        let version_blob = version_entries[0].2;
        let declared_len = u16::from_le_bytes(version_blob[0..2].try_into().unwrap()) as usize;
        assert_eq!(declared_len, version_blob.len());
        assert_eq!(
            u32::from_le_bytes(version_blob[40..44].try_into().unwrap()),
            0xFEEF_04BD
        );
        let utf16: Vec<u16> = version_blob
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&chunk| u16::from_le_bytes(chunk))
            .collect();
        let decoded = String::from_utf16_lossy(&utf16);
        assert!(decoded.contains("FileDescription"));
        assert!(decoded.contains("Poratake"));
        assert!(decoded.contains("Porabuild"));
        assert!(decoded.contains("poratake-gpui.exe"));
        assert!(decoded.contains("Copyright \u{00A9} 2026 Capty"));
    }
}
